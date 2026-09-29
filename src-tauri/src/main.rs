// Rust (Tauri) back end for the ID Card Generator - a one-to-one port of main.js from the Electron app.
// The UI in ../ui is the same HTML/CSS/JS; it calls these commands through window.__TAURI__.core.invoke.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod cities;
mod db;
mod format;
mod images;
mod paths;

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use serde_json::{json, Value};
use std::fs;
use std::path::{Component, Path, PathBuf};
use tauri::{Manager, State};

use db::{Db, IdRecord};
use format::{format_address_line1, format_contact_number};
use paths::Paths;

struct AppState {
    db: Db,
    paths: Paths,
}

// ---------- helpers for the loosely typed data object sent by the UI ----------

// data.key as a string ("" when missing, null or not a string - like `data.key || ''`)
fn text<'a>(data: &'a Value, key: &str) -> &'a str {
    data.get(key).and_then(Value::as_str).unwrap_or("")
}

// JavaScript truthiness for flags such as data.isRehire
fn flag(data: &Value, key: &str) -> bool {
    match data.get(key) {
        Some(Value::Bool(b)) => *b,
        Some(Value::String(s)) => !s.is_empty(),
        Some(Value::Number(n)) => n.as_f64().is_some_and(|f| f != 0.0),
        Some(Value::Array(_)) | Some(Value::Object(_)) => true,
        _ => false,
    }
}

// JavaScript String(value), used for PIN comparison
fn js_string(value: Option<&Value>) -> String {
    match value {
        None => "undefined".into(),
        Some(Value::Null) => "null".into(),
        Some(Value::String(s)) => s.clone(),
        Some(other) => other.to_string(),
    }
}

// ---------- PIN lock (data/pin-config.json, same format as the Electron app's src/pin-config.json) ----------

fn load_pin_config(state: &AppState) -> Value {
    fs::read_to_string(&state.paths.pin_config)
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .filter(Value::is_object)
        .unwrap_or_else(|| json!({ "pin": null, "setupComplete": false }))
}

fn save_pin_config(state: &AppState, config: &Value) -> Result<(), String> {
    let text = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
    fs::write(&state.paths.pin_config, text).map_err(|e| e.to_string())
}

#[tauri::command]
fn check_pin_setup(state: State<AppState>) -> Value {
    let config = load_pin_config(&state);
    json!({ "setupComplete": config.get("setupComplete").cloned().unwrap_or(Value::Bool(false)) })
}

#[tauri::command]
fn setup_pin(state: State<AppState>, pin: Value) -> Value {
    let mut config = load_pin_config(&state);
    config["pin"] = Value::String(js_string(Some(&pin)));
    config["setupComplete"] = Value::Bool(true);
    match save_pin_config(&state, &config) {
        Ok(()) => json!({ "success": true }),
        Err(error) => {
            eprintln!("setup-pin error: {error}");
            json!({ "success": false, "error": error })
        }
    }
}

#[tauri::command]
fn verify_pin(state: State<AppState>, pin: Value) -> Value {
    let config = load_pin_config(&state);
    json!({ "success": js_string(Some(&pin)) == js_string(config.get("pin")) })
}

// ---------- cities, control numbers, records ----------

#[tauri::command]
fn get_cities(state: State<AppState>) -> Vec<Value> {
    state.db.get_all_cities().unwrap_or_else(|err| {
        eprintln!("get-cities error: {err}");
        Vec::new()
    })
}

#[tauri::command]
fn get_city_code(state: State<AppState>, city_name: Option<String>) -> Option<String> {
    state.db.get_city_code(city_name.as_deref().unwrap_or("")).unwrap_or_else(|err| {
        eprintln!("get-city-code error: {err}");
        None
    })
}

#[tauri::command]
fn generate_control_number(state: State<AppState>, prefix: String, is_rehire: bool) -> Result<Value, String> {
    let control_number = state.db.generate_control_number(&prefix, is_rehire)?;
    Ok(json!({ "controlNumber": control_number }))
}

#[tauri::command]
fn get_all_records(state: State<AppState>) -> Vec<Value> {
    state.db.get_all_id_records().unwrap_or_else(|err| {
        eprintln!("get-all-records error: {err}");
        Vec::new()
    })
}

#[tauri::command]
fn reset_database(state: State<AppState>) -> Value {
    match state.db.reset_database() {
        Ok(()) => json!({ "success": true }),
        Err(err) => json!({ "success": false, "error": err.to_string() }),
    }
}

// ---------- ID card generation ----------

// Authorized representative's signature printed on the back ID (kept in the project root so it can be swapped)
fn load_authorized_signature(state: &AppState) -> Result<String, String> {
    let path = &state.paths.authorized_signature;
    let image = fs::read(path).map_err(|_| format!("Authorized signature not found: {}", path.display()))?;
    Ok(format!("data:image/png;base64,{}", BASE64.encode(image)))
}

// Sequence number from a control number such as 260627ANG-0950 or 260627ANG-0950-RH
fn control_sequence(control_number: &str) -> u32 {
    let base = control_number.strip_suffix("-RH").unwrap_or(control_number);
    base.rsplit('-')
        .next()
        .filter(|seq| seq.len() == 4 && seq.chars().all(|c| c.is_ascii_digit()))
        .and_then(|seq| seq.parse().ok())
        .unwrap_or(0)
}

fn generate_card(state: &AppState, data: &Value) -> Result<Value, String> {
    let db = &state.db;
    let err = |e: rusqlite::Error| e.to_string();
    let template;
    let replacements: Vec<(&str, String)>;
    let folder: String;
    let file_name;
    let mut issued_control_number = Value::Null;

    if flag(data, "isFront") {
        template = &state.paths.front_template;
        let (last_name, first_name) = (text(data, "lastName"), text(data, "firstName"));
        let (suffix, middle_initial) = (text(data, "suffix"), text(data, "middleInitial"));
        let last_name_with_suffix = if suffix.is_empty() { last_name.to_string() } else { format!("{last_name} {suffix}") };
        let first_name_with_middle =
            if middle_initial.is_empty() { first_name.to_string() } else { format!("{first_name} {middle_initial}") };

        let (hire_date, city_of_birth) = (text(data, "hireDate"), text(data, "cityOfBirth"));
        if hire_date.is_empty() || city_of_birth.is_empty() {
            return Err("Enter Hire Date and City of Birth to generate the control number".into());
        }
        let city_code = db
            .get_city_code(city_of_birth)
            .map_err(err)?
            .ok_or_else(|| format!("No city code found for {city_of_birth}"))?;
        let is_rehire = flag(data, "isRehire");
        // Prefix is YYMMDD + city code, e.g. 2026-06-27 in Angeles -> 260627ANG
        let prefix = format!("{}{city_code}", hire_date.replace('-', "").chars().skip(2).collect::<String>());

        // Reuse the number already issued in this session when regenerating, so each ID keeps one number
        let mut control_number = text(data, "controlNumber").to_string();
        if control_number.is_empty()
            || !control_number.starts_with(&format!("{prefix}-"))
            || control_number.ends_with("-RH") != is_rehire
        {
            control_number = db.generate_control_number(&prefix, is_rehire)?;
            db.reserve_control_number(&prefix, control_sequence(&control_number), is_rehire).map_err(err)?;
        }

        replacements = vec![
            // Box sizes match the <image> elements in templates/front-id.svg
            ("{{ID_PICTURE}}", images::photo_for_box(text(data, "idPicture"), 972.0, 982.0)),
            ("{{SIGNATURE_PICTURE}}", images::signature_for_box(text(data, "signaturePicture"), 1125.0, 250.0)),
            ("{{CONTROL_NUMBER}}", control_number.clone()),
            ("{{LAST_NAME}}", last_name_with_suffix.to_uppercase()),
            ("{{FIRST_NAME}}", first_name_with_middle.to_uppercase()),
            ("{{POSITION}}", text(data, "position").to_string()),
        ];
        folder = format!("{control_number}_{last_name}_{first_name}");
        file_name = "front-id.svg";
        issued_control_number = Value::String(control_number.clone());

        db.save_id_record(
            "front",
            &IdRecord {
                first_name,
                last_name,
                middle_initial,
                suffix,
                position: text(data, "position"),
                hire_date,
                city_of_birth,
                city_code: &city_code,
                is_rehire,
                control_number: &control_number,
                file_path: &format!("{folder}/{file_name}"),
                ..Default::default()
            },
        )
        .map_err(err)?;
    } else {
        template = &state.paths.back_template;
        // Control number is issued with the front ID; the back is filed under the same number
        let control_number = text(data, "controlNumber");
        if control_number.is_empty() {
            return Err("Download the Front ID first - the back is filed under its control number".into());
        }
        let city_of_birth = text(data, "cityOfBirth");
        let city_code = if city_of_birth.is_empty() { None } else { db.get_city_code(city_of_birth).map_err(err)? };
        let authorized_signature = load_authorized_signature(state)?;
        let (address1, address2) = (text(data, "addressLine1"), text(data, "addressLine2"));
        folder = format!("{control_number}_{}_{}", text(data, "surname"), text(data, "firstName"));
        file_name = "back-id.svg";

        db.save_id_record(
            "back",
            &IdRecord {
                first_name: text(data, "firstName"),
                last_name: text(data, "surname"),
                employee_name: text(data, "name"),
                hire_date: text(data, "hireDate"),
                city_of_birth,
                city_code: city_code.as_deref().unwrap_or(""),
                address1,
                address2,
                relationship: text(data, "relationship"),
                contact: text(data, "contact"),
                is_rehire: flag(data, "isRehire"),
                control_number,
                file_path: &format!("{folder}/{file_name}"),
                ..Default::default()
            },
        )
        .map_err(err)?;

        replacements = vec![
            ("{{NAME}}", text(data, "name").to_string()),
            ("{{RELATIONSHIP}}", text(data, "relationship").to_string()),
            ("{{ADDRESS}}", format_address_line1(address1, address2)),
            ("{{ADDRESS_LINE2}}", address2.to_string()),
            ("{{CONTACT_NO}}", format_contact_number(text(data, "contact"))),
            ("{{AUTHORIZED_SIGNATURE}}", authorized_signature),
        ];
    }

    let mut svg = fs::read_to_string(template).map_err(|e| format!("{e}: {}", template.display()))?;
    for (key, value) in &replacements {
        svg = svg.replace(key, value);
    }

    let output_dir = state.paths.id_output.join(&folder);
    fs::create_dir_all(&output_dir).map_err(|e| e.to_string())?;
    let save_path = output_dir.join(file_name);
    fs::write(&save_path, &svg).map_err(|e| e.to_string())?;

    Ok(json!({
        "success": true,
        "svgData": format!("data:image/svg+xml;base64,{}", BASE64.encode(svg.as_bytes())),
        "savePath": save_path.display().to_string(),
        "controlNumber": issued_control_number,
    }))
}

#[tauri::command]
fn generate_svg(state: State<AppState>, data: Value) -> Value {
    generate_card(&state, &data).unwrap_or_else(|error| json!({ "success": false, "error": error }))
}

// ---------- viewing generated cards from Records ----------

// A stored file_path may only point inside ID/ (plain folder/file names, no "..", drive or root)
fn is_safe_relative(path: &Path) -> bool {
    path.components().all(|c| matches!(c, Component::Normal(_)))
}

// The saved SVG for one side of a card: the path recorded in the database, or - for cards made before
// paths were recorded, or folders that were renamed - the newest <control number>_*/<file> under ID/
fn find_card_file(state: &AppState, control_number: &str, record_type: &str, file_name: &str) -> Option<PathBuf> {
    let id_output = &state.paths.id_output;
    if let Ok(Some(relative)) = state.db.latest_card_file(control_number, record_type) {
        let relative = PathBuf::from(relative);
        if is_safe_relative(&relative) && id_output.join(&relative).is_file() {
            return Some(id_output.join(relative));
        }
    }
    let folder_prefix = format!("{control_number}_");
    fs::read_dir(id_output)
        .ok()?
        .flatten()
        .filter(|entry| entry.file_name().to_string_lossy().starts_with(&folder_prefix))
        .map(|entry| entry.path().join(file_name))
        .filter_map(|path| Some((fs::metadata(&path).ok()?.modified().ok()?, path)))
        .max_by_key(|(modified, _)| *modified)
        .map(|(_, path)| path)
}

fn card_side(path: Option<PathBuf>) -> Value {
    let Some(path) = path else { return Value::Null };
    match fs::read(&path) {
        Ok(svg) => json!({
            "svgData": format!("data:image/svg+xml;base64,{}", BASE64.encode(svg)),
            "path": path.display().to_string(),
        }),
        Err(_) => Value::Null,
    }
}

#[tauri::command]
fn get_card_preview(state: State<AppState>, control_number: String) -> Value {
    if control_number.trim().is_empty() {
        return json!({ "success": false, "error": "This record has no control number" });
    }
    json!({
        "success": true,
        "front": card_side(find_card_file(&state, &control_number, "front", "front-id.svg")),
        "back": card_side(find_card_file(&state, &control_number, "back", "back-id.svg")),
    })
}

#[tauri::command]
fn open_card_folder(state: State<AppState>, control_number: String) -> Value {
    let folder = find_card_file(&state, &control_number, "front", "front-id.svg")
        .or_else(|| find_card_file(&state, &control_number, "back", "back-id.svg"))
        .and_then(|file| file.parent().map(Path::to_path_buf));
    let Some(folder) = folder else {
        return json!({ "success": false, "error": "Card folder not found in the ID folder" });
    };
    #[cfg(windows)]
    let opener = "explorer";
    #[cfg(target_os = "macos")]
    let opener = "open";
    #[cfg(all(not(windows), not(target_os = "macos")))]
    let opener = "xdg-open";
    // explorer.exe reports a non-zero exit code even when it opens the folder, so only a failed launch is an error
    match std::process::Command::new(opener).arg(&folder).spawn() {
        Ok(_) => json!({ "success": true }),
        Err(err) => json!({ "success": false, "error": err.to_string() }),
    }
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let paths = Paths::new(paths::base_dir());
            fs::create_dir_all(&paths.data_dir)?;
            let db = Db::open(&paths.database)?;
            app.manage(AppState { db, paths });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            check_pin_setup,
            setup_pin,
            verify_pin,
            get_cities,
            get_city_code,
            generate_control_number,
            get_all_records,
            reset_database,
            generate_svg,
            get_card_preview,
            open_card_folder
        ])
        .run(tauri::generate_context!())
        .expect("error while running ID Card Generator");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sequence_from_control_number() {
        assert_eq!(control_sequence("260627ANG-0950"), 950);
        assert_eq!(control_sequence("260627ANG-0950-RH"), 950);
        assert_eq!(control_sequence("20260627ALM-4728-RH"), 4728);
    }

    // Parity check against the Electron app: OSSI_PARITY_BASE is a throwaway project folder
    // (templates/, VP-SIGNATURE.png) and OSSI_PARITY_CASES a JSON list of {label, data} inputs.
    // Run with: cargo test parity -- --ignored
    #[test]
    #[ignore]
    fn parity() {
        let base = std::path::PathBuf::from(std::env::var("OSSI_PARITY_BASE").expect("OSSI_PARITY_BASE"));
        let cases: Vec<Value> =
            serde_json::from_str(&fs::read_to_string(std::env::var("OSSI_PARITY_CASES").expect("OSSI_PARITY_CASES")).unwrap())
                .unwrap();
        let paths = Paths::new(base.clone());
        fs::create_dir_all(&paths.data_dir).unwrap();
        let state = AppState { db: Db::open(&paths.database).unwrap(), paths };
        let mut results: Vec<Value> = cases
            .iter()
            .map(|c| {
                let mut r = generate_card(&state, &c["data"]).unwrap_or_else(|error| json!({ "success": false, "error": error }));
                r.as_object_mut().unwrap().insert("label".into(), c["label"].clone());
                r
            })
            .collect();
        results.push(json!({ "label": "records", "records": state.db.get_all_id_records().unwrap() }));
        fs::write(base.join("results.json"), serde_json::to_string_pretty(&results).unwrap()).unwrap();
    }

    #[test]
    fn stored_card_paths_stay_inside_id_folder() {
        assert!(is_safe_relative(Path::new("260627ANG-0950_DELA CRUZ_JUAN/front-id.svg")));
        assert!(!is_safe_relative(Path::new("../data/id-generator.db")));
        assert!(!is_safe_relative(Path::new("C:/Windows/win.ini")));
        assert!(!is_safe_relative(Path::new("/etc/passwd")));
    }

    #[test]
    fn js_truthiness() {
        let data = json!({ "a": true, "b": false, "c": "", "d": "x", "e": 0, "f": 1 });
        assert!(flag(&data, "a") && !flag(&data, "b") && !flag(&data, "c") && flag(&data, "d"));
        assert!(!flag(&data, "e") && flag(&data, "f") && !flag(&data, "missing"));
        assert_eq!(js_string(Some(&json!(1234))), "1234");
        assert_eq!(js_string(Some(&json!("1234"))), "1234");
        assert_eq!(js_string(None), "undefined");
    }
}
