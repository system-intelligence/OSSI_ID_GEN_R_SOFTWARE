// Correcting a card after it was generated (a misspelled name, a wrong contact number...).
//
// Rules: the control number never changes (it is the card's identity: QR code, folder, print log), so hire date
// and city of birth - which make up that number - are locked. A card sent to another PC for printing cannot be
// edited here. The photo and signatures are recovered from the saved card, so nothing has to be uploaded again.
// Saving regenerates both sides in place and records every changed field (from -> to) in card_edits.
use serde_json::{json, Map, Value};
use std::fs;
use std::path::PathBuf;

use crate::AppState;

fn value_text(record: &Map<String, Value>, key: &str) -> String {
    match record.get(key) {
        Some(Value::String(s)) => s.trim().to_string(),
        Some(Value::Number(n)) => n.to_string(),
        _ => String::new(),
    }
}

// xlink:href of the <image id="..."> in a saved card, e.g. the employee photo ("id-pic")
pub fn image_href(svg: &str, id: &str) -> Option<String> {
    let start = svg.find(&format!("id=\"{id}\""))?;
    let rest = &svg[start..];
    let element_end = rest.find("/>")?;
    let element = &rest[..element_end];
    let attr = "xlink:href=\"";
    let value_start = element.find(attr)? + attr.len();
    let value_end = element[value_start..].find('"')? + value_start;
    Some(element[value_start..value_end].to_string())
}

struct CardState {
    front: Map<String, Value>,
    back: Option<Map<String, Value>>,
    front_file: Option<PathBuf>,
    back_file: Option<PathBuf>,
    id_picture: String,
    signature_picture: String,
    sent_out: bool,
    times_printed: usize,
}

fn current_card(state: &AppState, control_number: &str) -> Result<CardState, String> {
    let db = &state.db;
    let err = |e: rusqlite::Error| e.to_string();
    let front = db
        .latest_card_record(control_number, "front")
        .map_err(err)?
        .ok_or_else(|| "This card has no front record, so it cannot be edited".to_string())?;
    let back = db.latest_card_record(control_number, "back").map_err(err)?;
    let front_file = crate::find_card_file(state, control_number, "front", "front-id.svg");
    let back_file = crate::find_card_file(state, control_number, "back", "back-id.svg");
    let front_svg = front_file.as_ref().and_then(|p| fs::read_to_string(p).ok()).unwrap_or_default();
    Ok(CardState {
        id_picture: image_href(&front_svg, "id-pic").unwrap_or_default(),
        signature_picture: image_href(&front_svg, "signature-img").unwrap_or_default(),
        sent_out: db.card_transfers(control_number).map_err(err)?.iter().any(|t| t["direction"] == "out"),
        times_printed: db.print_history(control_number).map_err(err)?.len(),
        front,
        back,
        front_file,
        back_file,
    })
}

const SENT_OUT: &str = "This card was sent to another PC for printing, so it can only be corrected on that PC";

// Everything the edit form needs, read from the card as it is now
pub fn load(state: &AppState, control_number: &str) -> Result<Value, String> {
    let card = current_card(state, control_number)?;
    let f = &card.front;
    Ok(json!({
        "controlNumber": control_number,
        "sentOut": card.sent_out,
        "sentOutMessage": if card.sent_out { SENT_OUT } else { "" },
        "timesPrinted": card.times_printed,
        "locked": {
            "hireDate": value_text(f, "hire_date"),
            "cityOfBirth": value_text(f, "city_of_birth"),
            "isRehire": value_text(f, "is_rehire") == "1",
        },
        "front": {
            "lastName": value_text(f, "last_name"),
            "firstName": value_text(f, "first_name"),
            "middleInitial": value_text(f, "middle_initial"),
            "suffix": value_text(f, "suffix"),
            "position": value_text(f, "position"),
        },
        "back": card.back.as_ref().map(|b| json!({
            "emergencyName": value_text(b, "employee_name"),
            "relationship": value_text(b, "relationship"),
            "address1": value_text(b, "address1"),
            "address2": value_text(b, "address2"),
            "contact": value_text(b, "contact"),
        })),
        "idPicture": card.id_picture,
        "signaturePicture": card.signature_picture,
    }))
}

// Applies the corrections in `changes` (same keys as load() returns, plus optional new idPicture /
// signaturePicture data URLs) and regenerates the card under the same control number
pub fn save(state: &AppState, control_number: &str, changes: &Value) -> Result<Value, String> {
    let card = current_card(state, control_number)?;
    if card.sent_out {
        return Err(SENT_OUT.into());
    }
    let f = &card.front;
    let (hire_date, city_of_birth) = (value_text(f, "hire_date"), value_text(f, "city_of_birth"));
    let is_rehire = value_text(f, "is_rehire") == "1";

    // The regenerated card must keep this exact control number
    let city_code = state
        .db
        .get_place_code(&city_of_birth)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("No code found for place of birth {city_of_birth}"))?;
    let prefix = format!("{}{city_code}-", hire_date.replace('-', "").chars().skip(2).collect::<String>());
    if !control_number.starts_with(&prefix) || control_number.ends_with("-RH") != is_rehire {
        return Err("This card's control number does not match its hire date and city, so it cannot be edited".into());
    }

    // New value of a field: the edited one if given, otherwise the current one
    let new = |key: &str, current: String, upper: bool| -> String {
        let value = changes.get(key).and_then(Value::as_str).map(|s| s.split_whitespace().collect::<Vec<_>>().join(" "));
        let value = value.unwrap_or(current);
        if upper { value.to_uppercase() } else { value }
    };
    let front_fields = [
        ("lastName", "Last name", "last_name", true),
        ("firstName", "First name", "first_name", true),
        ("middleInitial", "Middle initial", "middle_initial", true),
        ("suffix", "Suffix", "suffix", true),
        ("position", "Position", "position", true),
    ];
    let back_fields = [
        ("emergencyName", "Emergency contact name", "employee_name"),
        ("relationship", "Relationship", "relationship"),
        ("address1", "Address line 1", "address1"),
        ("address2", "Address line 2", "address2"),
        ("contact", "Contact number", "contact"),
    ];

    let mut edits: Vec<(String, String, String)> = Vec::new();
    let mut front_values = Map::new();
    for (key, label, column, upper) in front_fields {
        let (old, value) = (value_text(f, column), new(key, value_text(f, column), upper));
        if old != value {
            edits.push((label.to_string(), old, value.clone()));
        }
        front_values.insert(key.to_string(), Value::String(value));
    }
    if front_values["lastName"].as_str().unwrap_or("").is_empty() || front_values["firstName"].as_str().unwrap_or("").is_empty() {
        return Err("Last name and first name cannot be empty".into());
    }
    if front_values["middleInitial"].as_str().unwrap_or("").chars().count() > 3 {
        return Err("Middle initial can be at most 3 characters".into());
    }
    let mut back_values = Map::new();
    if let Some(b) = &card.back {
        for (key, label, column) in back_fields {
            let (old, value) = (value_text(b, column), new(key, value_text(b, column), false));
            if old != value {
                edits.push((label.to_string(), old, value.clone()));
            }
            back_values.insert(key.to_string(), Value::String(value));
        }
        for key in ["address1", "address2"] {
            if back_values[key].as_str().unwrap_or("").chars().count() > 29 {
                return Err("Address lines can be at most 29 characters".into());
            }
        }
    }
    let replaced = |key: &str| changes.get(key).and_then(Value::as_str).filter(|s| s.starts_with("data:image/"));
    let id_picture = replaced("idPicture").map(str::to_string);
    let signature_picture = replaced("signaturePicture").map(str::to_string);
    if id_picture.is_some() {
        edits.push(("ID photo".into(), "previous photo".into(), "new photo".into()));
    }
    if signature_picture.is_some() {
        edits.push(("Signature".into(), "previous signature".into(), "new signature".into()));
    }
    if edits.is_empty() {
        return Ok(json!({ "success": true, "changed": [], "needsReprint": false }));
    }
    if card.back.is_some() {
        crate::load_authorized_signature(state)?; // fail before touching anything
    }

    // Regenerate the front, then the back (its folder follows the front names)
    let text = |m: &Map<String, Value>, k: &str| m.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    let front_data = json!({
        "isFront": true,
        "controlNumber": control_number,
        "lastName": text(&front_values, "lastName"),
        "firstName": text(&front_values, "firstName"),
        "middleInitial": text(&front_values, "middleInitial"),
        "suffix": text(&front_values, "suffix"),
        "position": text(&front_values, "position"),
        "hireDate": hire_date,
        "cityOfBirth": city_of_birth,
        "isRehire": is_rehire,
        "idPicture": id_picture.unwrap_or(card.id_picture),
        "signaturePicture": signature_picture.unwrap_or(card.signature_picture),
    });
    let result = crate::generate_card(state, &front_data)?;
    if result["controlNumber"].as_str() != Some(control_number) {
        return Err("The control number changed while saving - please report this".into());
    }
    if card.back.is_some() {
        let back_data = json!({
            "isFront": false,
            "controlNumber": control_number,
            "name": text(&back_values, "emergencyName"),
            "relationship": text(&back_values, "relationship"),
            "addressLine1": text(&back_values, "address1"),
            "addressLine2": text(&back_values, "address2"),
            "contact": text(&back_values, "contact"),
            "surname": text(&front_values, "lastName"),
            "firstName": text(&front_values, "firstName"),
            "hireDate": hire_date,
            "cityOfBirth": city_of_birth,
            "isRehire": is_rehire,
        });
        crate::generate_card(state, &back_data)?;
    }

    // A changed name means a new folder name: remove the outdated files so only the corrected card remains
    for (old, side, name) in [(&card.front_file, "front", "front-id.svg"), (&card.back_file, "back", "back-id.svg")] {
        let current = crate::find_card_file(state, control_number, side, name);
        if let (Some(old), Some(current)) = (old, current) {
            if *old != current {
                let _ = fs::remove_file(old);
                if let Some(folder) = old.parent() {
                    let _ = fs::remove_dir(folder); // only succeeds once the folder is empty
                }
            }
        }
    }

    state.db.log_card_edits(control_number, &edits).map_err(|e| e.to_string())?;
    let changed: Vec<Value> =
        edits.iter().map(|(field, old, new)| json!({ "field": field, "oldValue": old, "newValue": new })).collect();
    Ok(json!({ "success": true, "changed": changed, "needsReprint": card.times_printed > 0 }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn photo_and_signature_are_found_in_a_saved_card() {
        let svg = r#"<svg><image id="id-pic" x="574" preserveAspectRatio="none"
                xlink:href="data:image/jpeg;base64,AAA"
                />
            <image id="signature-img" x="500"
                xlink:href="data:image/png;base64,BBB"
                /></svg>"#;
        assert_eq!(image_href(svg, "id-pic").as_deref(), Some("data:image/jpeg;base64,AAA"));
        assert_eq!(image_href(svg, "signature-img").as_deref(), Some("data:image/png;base64,BBB"));
        assert_eq!(image_href(svg, "logo-img"), None);
    }
}
