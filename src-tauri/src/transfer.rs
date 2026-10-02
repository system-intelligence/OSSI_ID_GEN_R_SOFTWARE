// "Send to another PC for printing" files (.ossi): cards that PC A has not printed yet, packed with their
// records and saved card SVGs, encrypted with a password so the personal data and signatures inside are
// unreadable without it.
//
// File layout:  "OSSIXFER" | version (1 byte) | salt (16) | nonce (12) | AES-256-GCM ciphertext
// The key is derived from the password with Argon2id (slow on purpose, so guessing passwords is impractical).
// GCM also authenticates: a wrong password or any change to the file is detected instead of producing garbage.
use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Nonce};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

const MAGIC: &[u8; 8] = b"OSSIXFER";
const VERSION: u8 = 1;
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;
const HEADER_LEN: usize = MAGIC.len() + 1 + SALT_LEN + NONCE_LEN;
pub const FORMAT: &str = "ossi-print-transfer";
pub const MIN_PASSWORD_LEN: usize = 8;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Package {
    pub format: String,
    pub version: u32,
    pub exported_at: String,
    pub source_pc: String,
    pub cards: Vec<Card>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Card {
    pub control_number: String,
    pub records: Vec<Map<String, Value>>,
    pub reservations: Vec<Map<String, Value>>,
    pub files: Vec<CardFile>,
}

// A saved card side: ID/<folder>/<name>
#[derive(Serialize, Deserialize)]
pub struct CardFile {
    pub folder: String,
    pub name: String,
    pub svg: String,
}

fn derive_key(password: &str, salt: &[u8]) -> Result<[u8; 32], String> {
    let mut key = [0u8; 32];
    argon2::Argon2::default()
        .hash_password_into(password.as_bytes(), salt, &mut key)
        .map_err(|e| format!("Could not derive the encryption key: {e}"))?;
    Ok(key)
}

pub fn encrypt(plain: &[u8], password: &str) -> Result<Vec<u8>, String> {
    let mut salt = [0u8; SALT_LEN];
    let mut nonce = [0u8; NONCE_LEN];
    getrandom::getrandom(&mut salt).map_err(|e| e.to_string())?;
    getrandom::getrandom(&mut nonce).map_err(|e| e.to_string())?;
    let key = derive_key(password, &salt)?;
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|e| e.to_string())?;

    let mut out = Vec::with_capacity(HEADER_LEN + plain.len() + 16);
    out.extend_from_slice(MAGIC);
    out.push(VERSION);
    out.extend_from_slice(&salt);
    out.extend_from_slice(&nonce);
    // The header is authenticated too, so it cannot be swapped or altered
    let ciphertext = cipher
        .encrypt(Nonce::from_slice(&nonce), Payload { msg: plain, aad: &out[..HEADER_LEN] })
        .map_err(|_| "Encryption failed".to_string())?;
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

pub fn decrypt(data: &[u8], password: &str) -> Result<Vec<u8>, String> {
    if data.len() <= HEADER_LEN || &data[..MAGIC.len()] != MAGIC {
        return Err("This is not an OSSI transfer file (.ossi)".into());
    }
    if data[MAGIC.len()] != VERSION {
        return Err("This transfer file was made by a newer version of the app - update the app on this PC".into());
    }
    let salt = &data[MAGIC.len() + 1..MAGIC.len() + 1 + SALT_LEN];
    let nonce = &data[MAGIC.len() + 1 + SALT_LEN..HEADER_LEN];
    let key = derive_key(password, salt)?;
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|e| e.to_string())?;
    cipher
        .decrypt(Nonce::from_slice(nonce), Payload { msg: &data[HEADER_LEN..], aad: &data[..HEADER_LEN] })
        .map_err(|_| "Wrong password, or the file is damaged".to_string())
}

// Only "front-id.svg" / "back-id.svg" inside one plain folder name may be written on import
pub fn is_safe_card_file(file: &CardFile) -> bool {
    let folder = std::path::Path::new(&file.folder);
    let single_folder = folder.components().count() == 1
        && matches!(folder.components().next(), Some(std::path::Component::Normal(_)));
    single_folder
        && matches!(file.name.as_str(), "front-id.svg" | "back-id.svg")
        && file.svg.trim_start().starts_with("<svg")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_with_the_right_password() {
        let encrypted = encrypt(b"card data", "correct horse").unwrap();
        assert!(encrypted.starts_with(MAGIC));
        assert!(!encrypted.windows(9).any(|w| w == b"card data"), "plain text must not appear in the file");
        assert_eq!(decrypt(&encrypted, "correct horse").unwrap(), b"card data");
    }

    #[test]
    fn wrong_password_or_tampering_is_rejected() {
        let mut encrypted = encrypt(b"card data", "correct horse").unwrap();
        assert_eq!(decrypt(&encrypted, "wrong password").unwrap_err(), "Wrong password, or the file is damaged");
        let last = encrypted.len() - 1;
        encrypted[last] ^= 1;
        assert!(decrypt(&encrypted, "correct horse").is_err());
        assert!(decrypt(b"not a transfer file at all, just text", "x").unwrap_err().contains("not an OSSI"));
    }

    #[test]
    fn each_export_uses_fresh_salt_and_nonce() {
        assert_ne!(encrypt(b"same", "same password").unwrap(), encrypt(b"same", "same password").unwrap());
    }

    #[test]
    fn only_card_svgs_in_a_plain_folder_may_be_written() {
        let file = |folder: &str, name: &str, svg: &str| CardFile { folder: folder.into(), name: name.into(), svg: svg.into() };
        assert!(is_safe_card_file(&file("270223CAB-9671-RH_EDICA_JIRRUM", "front-id.svg", "<svg/>")));
        assert!(!is_safe_card_file(&file("../data", "front-id.svg", "<svg/>")));
        assert!(!is_safe_card_file(&file("a/b", "front-id.svg", "<svg/>")));
        assert!(!is_safe_card_file(&file("C:\\Windows", "front-id.svg", "<svg/>")));
        assert!(!is_safe_card_file(&file("ok", "id-generator.db", "<svg/>")));
        assert!(!is_safe_card_file(&file("ok", "back-id.svg", "MZ executable")));
    }
}
