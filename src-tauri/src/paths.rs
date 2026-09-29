use std::path::{Path, PathBuf};

// App folder that holds templates/, VP-SIGNATURE.png, data/ (database + PIN) and ID/ (generated cards).
// Set OSSI_BASE_DIR to override; otherwise the nearest folder above the executable containing
// templates/front-id.svg is used (during development that is the OSSI_ID_GEN_R_SOFTWARE folder).
pub fn base_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("OSSI_BASE_DIR") {
        if !dir.trim().is_empty() {
            return PathBuf::from(dir);
        }
    }
    let exe = std::env::current_exe().unwrap_or_default();
    exe.ancestors()
        .find(|dir| dir.join("templates").join("front-id.svg").is_file())
        .map(PathBuf::from)
        .or_else(|| exe.parent().map(PathBuf::from))
        .unwrap_or_default()
}

pub struct Paths {
    pub data_dir: PathBuf,
    pub database: PathBuf,
    pub pin_config: PathBuf,
    pub front_template: PathBuf,
    pub back_template: PathBuf,
    pub id_output: PathBuf,
    pub authorized_signature: PathBuf,
}

// data/<file>, unless only the Electron app's src/<file> exists (OSSI_BASE_DIR pointed at the old project)
fn data_file(base: &Path, name: &str) -> PathBuf {
    let current = base.join("data").join(name);
    let legacy = base.join("src").join(name);
    if !current.exists() && legacy.is_file() { legacy } else { current }
}

impl Paths {
    pub fn new(base: PathBuf) -> Self {
        Paths {
            data_dir: base.join("data"),
            database: data_file(&base, "id-generator.db"),
            pin_config: data_file(&base, "pin-config.json"),
            front_template: base.join("templates").join("front-id.svg"),
            back_template: base.join("templates").join("back-id.svg"),
            id_output: base.join("ID"),
            authorized_signature: base.join("VP-SIGNATURE.png"),
        }
    }
}
