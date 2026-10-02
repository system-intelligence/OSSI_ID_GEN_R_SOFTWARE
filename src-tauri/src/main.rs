// Rust (Tauri) back end for the ID Card Generator - a one-to-one port of main.js from the Electron app.
// The UI in ../ui is the same HTML/CSS/JS; it calls these commands through window.__TAURI__.core.invoke.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod cities;
mod db;
mod edit;
mod format;
mod images;
mod paths;
mod printer;
mod qr;
mod textfit;
mod transfer;

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

// Text typed in the form, made safe to put inside an SVG element ("A & B" would otherwise break the file)
fn xml_text(value: &str) -> String {
    value.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
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

// Authorized representative's signature printed on the back ID: VP-SIGNATURE.png in the working folder
// (so it can be swapped without reinstalling), else the copy bundled with the installer
fn load_authorized_signature(state: &AppState) -> Result<String, String> {
    let paths = &state.paths;
    let image = fs::read(&paths.authorized_signature)
        .or_else(|_| fs::read(&paths.bundled_signature))
        .map_err(|_| {
            format!("Authorized signature not found - copy VP-SIGNATURE.png to {}", paths.authorized_signature.display())
        })?;
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
            ("{{CONTROL_NUMBER}}", xml_text(&control_number)),
            ("{{LAST_NAME}}", xml_text(&last_name_with_suffix.to_uppercase())),
            ("{{FIRST_NAME}}", xml_text(&first_name_with_middle.to_uppercase())),
            ("{{POSITION}}", xml_text(text(data, "position"))),
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
            ("{{NAME}}", xml_text(text(data, "name"))),
            ("{{RELATIONSHIP}}", xml_text(text(data, "relationship"))),
            ("{{ADDRESS}}", xml_text(&format_address_line1(address1, address2))),
            ("{{ADDRESS_LINE2}}", xml_text(address2)),
            ("{{CONTACT_NO}}", xml_text(&format_contact_number(text(data, "contact")))),
            ("{{AUTHORIZED_SIGNATURE}}", authorized_signature),
            // QR of the card's control number, so scanning it identifies the employee record
            ("{{QR_CODE}}", qr::qr_svg_path(control_number, qr::QR_X, qr::QR_Y, qr::QR_SIZE)?),
        ];
    }

    let mut svg = fs::read_to_string(template).map_err(|e| format!("{e}: {}", template.display()))?;
    for (key, value) in &replacements {
        svg = svg.replace(key, value);
    }
    // Centred lines get an exact left position measured with the installed fonts, so CorelDRAW shows
    // them exactly where the browser does (see textfit.rs)
    let svg = textfit::left_align_centred_text_with_installed_fonts(&svg);

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

// ---------- print log ----------

#[tauri::command]
fn log_print(state: State<AppState>, control_number: String, side: String, reason: Option<String>, printer: Option<String>) -> Value {
    if control_number.trim().is_empty() || !matches!(side.as_str(), "front" | "back") {
        return json!({ "success": false, "error": "A print needs a control number and a side (front or back)" });
    }
    match state.db.log_print(&control_number, &side, reason.as_deref().unwrap_or(""), printer.as_deref().unwrap_or("")) {
        Ok(entry) => json!({ "success": true, "entry": entry }),
        Err(err) => json!({ "success": false, "error": err.to_string() }),
    }
}

#[tauri::command]
fn get_print_history(state: State<AppState>, control_number: String) -> Value {
    match (state.db.print_history(&control_number), state.db.card_transfers(&control_number)) {
        (Ok(history), Ok(transfers)) => json!({
            "success": true,
            "history": history,
            "transfers": transfers,
            "edits": state.db.card_edits(&control_number).unwrap_or_default(),
        }),
        (Err(err), _) | (_, Err(err)) => json!({ "success": false, "error": err.to_string() }),
    }
}

#[tauri::command]
fn get_print_log(state: State<AppState>) -> Value {
    match state.db.print_log_entries() {
        Ok(entries) => json!({ "success": true, "entries": entries }),
        Err(err) => json!({ "success": false, "error": err.to_string() }),
    }
}

#[tauri::command]
fn get_print_summary(state: State<AppState>) -> Value {
    state.db.print_summary().map(Value::Object).unwrap_or_else(|err| {
        eprintln!("get-print-summary error: {err}");
        json!({})
    })
}

// Asking Windows takes about a second, so it runs off the main thread to keep the window responsive
#[tauri::command]
async fn get_printer_status() -> Value {
    tauri::async_runtime::spawn_blocking(printer::status)
        .await
        .unwrap_or_else(|err| json!({ "state": "unknown", "error": err.to_string() }))
}

#[tauri::command]
fn open_card_folder(state: State<AppState>, control_number: String) -> Value {
    let folder = find_card_file(&state, &control_number, "front", "front-id.svg")
        .or_else(|| find_card_file(&state, &control_number, "back", "back-id.svg"))
        .and_then(|file| file.parent().map(Path::to_path_buf));
    let Some(folder) = folder else {
        return json!({ "success": false, "error": "Card folder not found in the ID folder" });
    };
    open_in_file_manager(&folder)
}

fn open_in_file_manager(folder: &Path) -> Value {
    #[cfg(windows)]
    let opener = "explorer";
    #[cfg(target_os = "macos")]
    let opener = "open";
    #[cfg(all(not(windows), not(target_os = "macos")))]
    let opener = "xdg-open";
    // explorer.exe reports a non-zero exit code even when it opens the folder, so only a failed launch is an error
    match std::process::Command::new(opener).arg(folder).spawn() {
        Ok(_) => json!({ "success": true }),
        Err(err) => json!({ "success": false, "error": err.to_string() }),
    }
}

// ---------- sending unprinted cards to another PC for printing ----------

fn this_pc_name() -> String {
    std::env::var("COMPUTERNAME").unwrap_or_else(|_| "another PC".into())
}

#[tauri::command]
fn get_export_candidates(state: State<AppState>) -> Value {
    match state.db.export_candidates() {
        Ok(cards) => json!({ "success": true, "cards": cards }),
        Err(err) => json!({ "success": false, "error": err.to_string() }),
    }
}

// Packs every card this PC has never printed (and never sent before) into one encrypted .ossi file in
// Documents\OSSI ID Generator\exports, then marks those cards "sent to other PC" so they are not printed here too.
fn export_cards(state: &AppState, password: &str) -> Result<Value, String> {
    if password.chars().count() < transfer::MIN_PASSWORD_LEN {
        return Err(format!("Use a password of at least {} characters", transfer::MIN_PASSWORD_LEN));
    }
    let candidates = state.db.export_candidates().map_err(|e| e.to_string())?;
    let mut cards = Vec::new();
    let mut skipped = Vec::new();
    for candidate in &candidates {
        let control = candidate["controlNumber"].as_str().unwrap_or_default().to_string();
        let mut files = Vec::new();
        for (side, name) in [("front", "front-id.svg"), ("back", "back-id.svg")] {
            if let Some(path) = find_card_file(state, &control, side, name) {
                let folder = path.parent().and_then(|p| p.file_name()).map(|f| f.to_string_lossy().into_owned());
                if let (Some(folder), Ok(svg)) = (folder, fs::read_to_string(&path)) {
                    files.push(transfer::CardFile { folder, name: name.to_string(), svg });
                }
            }
        }
        if files.is_empty() {
            // Without the saved SVGs the other PC could not print it, so it stays here
            skipped.push(json!({ "controlNumber": control, "name": candidate["name"], "reason": "card files not found" }));
            continue;
        }
        cards.push(transfer::Card {
            records: state.db.card_records(&control).map_err(|e| e.to_string())?,
            reservations: state.db.card_reservations(&control).map_err(|e| e.to_string())?,
            control_number: control,
            files,
        });
    }
    if cards.is_empty() {
        return Err("There are no unprinted cards to send".into());
    }

    let package = transfer::Package {
        format: transfer::FORMAT.into(),
        version: 1,
        exported_at: chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
        source_pc: this_pc_name(),
        cards,
    };
    let plain = serde_json::to_vec(&package).map_err(|e| e.to_string())?;
    let encrypted = transfer::encrypt(&plain, password)?;

    fs::create_dir_all(&state.paths.exports_dir).map_err(|e| e.to_string())?;
    let file_name = format!(
        "OSSI-print-transfer_{}_{}_{}-cards.ossi",
        package.source_pc,
        chrono::Local::now().format("%Y%m%d-%H%M%S"),
        package.cards.len()
    );
    let path = state.paths.exports_dir.join(file_name);
    fs::write(&path, encrypted).map_err(|e| e.to_string())?;

    // Only after the file is safely written: these cards are now the other PC's to print
    let sent: Vec<String> = package.cards.iter().map(|c| c.control_number.clone()).collect();
    state.db.mark_sent_out(&sent, "").map_err(|e| e.to_string())?;
    let names: Vec<Value> = package
        .cards
        .iter()
        .map(|c| {
            let name = candidates.iter().find(|k| k["controlNumber"] == c.control_number.as_str()).map(|k| k["name"].clone());
            json!({ "controlNumber": c.control_number, "name": name.unwrap_or(Value::Null) })
        })
        .collect();
    Ok(json!({ "success": true, "path": path.display().to_string(), "sent": names, "skipped": skipped }))
}

#[tauri::command]
fn export_for_printing(state: State<AppState>, password: String) -> Value {
    export_cards(&state, &password).unwrap_or_else(|error| json!({ "success": false, "error": error }))
}

// Adds the cards from a .ossi file. Never overwrites: a control number already used here is skipped
// (as "already on this PC" when it is the same person, otherwise reported as a conflict).
fn import_cards(state: &AppState, data_base64: &str, password: &str) -> Result<Value, String> {
    let data = BASE64.decode(data_base64.trim()).map_err(|_| "The selected file could not be read".to_string())?;
    let plain = transfer::decrypt(&data, password)?;
    let package: transfer::Package =
        serde_json::from_slice(&plain).map_err(|_| "The transfer file is damaged".to_string())?;
    if package.format != transfer::FORMAT {
        return Err("This is not an OSSI print transfer file".into());
    }

    let mut imported = Vec::new();
    let mut skipped = Vec::new();
    for card in &package.cards {
        let control = card.control_number.trim();
        let front = card.records.iter().find(|r| r.get("type").and_then(Value::as_str) == Some("front"));
        let name = front
            .map(|r| {
                ["first_name", "middle_initial", "last_name", "suffix"]
                    .iter()
                    .filter_map(|k| r.get(*k).and_then(Value::as_str))
                    .filter(|s| !s.trim().is_empty())
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .unwrap_or_default();
        if control.is_empty() || card.records.is_empty() {
            skipped.push(json!({ "controlNumber": control, "name": name, "reason": "incomplete card" }));
            continue;
        }
        if let Some(owner) = state.db.card_owner(control).map_err(|e| e.to_string())? {
            let incoming = front
                .map(|r| {
                    format!(
                        "{} {}",
                        r.get("first_name").and_then(Value::as_str).unwrap_or(""),
                        r.get("last_name").and_then(Value::as_str).unwrap_or("")
                    )
                    .trim()
                    .to_string()
                })
                .unwrap_or_default();
            let reason = if owner.eq_ignore_ascii_case(&incoming) {
                "already on this PC".to_string()
            } else {
                format!("control number already used here by {owner}")
            };
            skipped.push(json!({ "controlNumber": control, "name": name, "reason": reason }));
            continue;
        }
        if !card.files.iter().all(transfer::is_safe_card_file) {
            skipped.push(json!({ "controlNumber": control, "name": name, "reason": "unexpected file in transfer" }));
            continue;
        }
        for file in &card.files {
            let folder = state.paths.id_output.join(&file.folder);
            fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
            fs::write(folder.join(&file.name), &file.svg).map_err(|e| e.to_string())?;
        }
        state
            .db
            .import_card(control, &card.records, &card.reservations, &package.source_pc)
            .map_err(|e| e.to_string())?;
        imported.push(json!({ "controlNumber": control, "name": name }));
    }
    Ok(json!({
        "success": true,
        "sourcePc": package.source_pc,
        "exportedAt": package.exported_at,
        "imported": imported,
        "skipped": skipped,
    }))
}

#[tauri::command]
fn import_print_transfer(state: State<AppState>, data_base64: String, password: String) -> Value {
    import_cards(&state, &data_base64, &password).unwrap_or_else(|error| json!({ "success": false, "error": error }))
}

// ---------- correcting a saved card (see edit.rs) ----------

#[tauri::command]
fn get_card_for_edit(state: State<AppState>, control_number: String) -> Value {
    match edit::load(&state, &control_number) {
        Ok(card) => json!({ "success": true, "card": card }),
        Err(error) => json!({ "success": false, "error": error }),
    }
}

#[tauri::command]
fn update_card(state: State<AppState>, control_number: String, changes: Value) -> Value {
    edit::save(&state, &control_number, &changes).unwrap_or_else(|error| json!({ "success": false, "error": error }))
}

#[tauri::command]
fn open_exports_folder(state: State<AppState>) -> Value {
    if let Err(err) = fs::create_dir_all(&state.paths.exports_dir) {
        return json!({ "success": false, "error": err.to_string() });
    }
    open_in_file_manager(&state.paths.exports_dir)
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let (resources, work) = paths::locate(app.path().document_dir().ok());
            let paths = Paths::new(resources, work);
            fs::create_dir_all(&paths.data_dir)?;
            fs::create_dir_all(&paths.id_output)?;
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
            open_card_folder,
            get_printer_status,
            log_print,
            get_print_history,
            get_print_summary,
            get_print_log,
            get_export_candidates,
            export_for_printing,
            import_print_transfer,
            open_exports_folder,
            get_card_for_edit,
            update_card
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
        let paths = Paths::new(base.clone(), base.clone());
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

    // Two "PCs" (separate databases and ID folders) using the real templates
    fn test_pc(name: &str) -> (AppState, std::path::PathBuf) {
        let project = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let work = std::env::temp_dir().join(format!("ossi-transfer-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&work);
        let paths = Paths::new(project.to_path_buf(), work.clone());
        fs::create_dir_all(&paths.data_dir).unwrap();
        (AppState { db: Db::open(&paths.database).unwrap(), paths }, work)
    }

    // Generates the front and back of a card on `pc` and returns its control number
    fn make_card(pc: &AppState, first: &str, last: &str) -> String {
        let front = generate_card(
            pc,
            &json!({ "isFront": true, "firstName": first, "lastName": last, "position": "SECURITY GUARD",
                     "hireDate": "2026-06-27", "cityOfBirth": "ANGELES CITY" }),
        )
        .unwrap();
        let control = front["controlNumber"].as_str().unwrap().to_string();
        generate_card(
            pc,
            &json!({ "isFront": false, "controlNumber": control, "name": "Maria Cruz", "relationship": "Mother",
                     "addressLine1": "123 Rizal St", "addressLine2": "Quezon City", "contact": "09171234567",
                     "surname": last, "firstName": first, "hireDate": "2026-06-27", "cityOfBirth": "ANGELES CITY" }),
        )
        .unwrap();
        control
    }

    #[test]
    fn unprinted_cards_move_to_another_pc_once() {
        if !std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../VP-SIGNATURE.png").is_file() {
            return; // the back ID needs the signature, which is not in git
        }
        let (pc_a, work_a) = test_pc("a");
        let (pc_b, work_b) = test_pc("b");
        let unprinted = make_card(&pc_a, "JUAN", "DELA CRUZ");
        let printed = make_card(&pc_a, "PEDRO", "SANTOS");
        pc_a.db.log_print(&printed, "front", "", "SMART-51").unwrap();

        // Only the never-printed card is offered and sent
        let candidates: Vec<String> = pc_a.db.export_candidates().unwrap().iter()
            .map(|c| c["controlNumber"].as_str().unwrap().to_string()).collect();
        assert_eq!(candidates, vec![unprinted.clone()]);
        assert!(export_cards(&pc_a, "short").unwrap_err().contains("at least 8"));
        let exported = export_cards(&pc_a, "office-pass-2026").unwrap();
        assert_eq!(exported["sent"].as_array().unwrap().len(), 1);
        let file = std::path::PathBuf::from(exported["path"].as_str().unwrap());
        let bytes = fs::read(&file).unwrap();
        assert!(!String::from_utf8_lossy(&bytes).contains("DELA CRUZ"), "names must not be readable in the file");

        // PC A: the card is now marked as sent, and it cannot be sent again
        assert!(pc_a.db.export_candidates().unwrap().is_empty());
        assert_eq!(pc_a.db.card_transfers(&unprinted).unwrap()[0]["direction"], "out");
        assert!(export_cards(&pc_a, "office-pass-2026").unwrap_err().contains("no unprinted cards"));

        // PC B: wrong password gets nothing; the right one adds the card with its files
        let data = BASE64.encode(&bytes);
        assert_eq!(import_cards(&pc_b, &data, "wrong-password").unwrap_err(), "Wrong password, or the file is damaged");
        assert!(pc_b.db.card_owner(&unprinted).unwrap().is_none());
        let imported = import_cards(&pc_b, &data, "office-pass-2026").unwrap();
        assert_eq!(imported["imported"][0]["controlNumber"], unprinted.as_str());
        assert_eq!(imported["imported"][0]["name"], "JUAN DELA CRUZ");
        assert!(find_card_file(&pc_b, &unprinted, "front", "front-id.svg").is_some());
        assert!(find_card_file(&pc_b, &unprinted, "back", "back-id.svg").is_some());
        assert_eq!(pc_b.db.card_records(&unprinted).unwrap().len(), 2);
        assert_eq!(pc_b.db.card_transfers(&unprinted).unwrap()[0]["direction"], "in");
        // ...and B never re-issues that control number
        assert_eq!(pc_b.db.card_reservations(&unprinted).unwrap().len(), 1);

        // Importing the same file again adds nothing
        let again = import_cards(&pc_b, &data, "office-pass-2026").unwrap();
        assert!(again["imported"].as_array().unwrap().is_empty());
        assert_eq!(again["skipped"][0]["reason"], "already on this PC");

        // A received card is never sent onward from B
        assert!(pc_b.db.export_candidates().unwrap().is_empty());

        // close the databases first: Windows cannot delete a file that is still open
        drop((pc_a, pc_b));
        let _ = fs::remove_dir_all(work_a);
        let _ = fs::remove_dir_all(work_b);
    }

    #[test]
    fn correcting_a_printed_card_keeps_its_number_and_photo() {
        if !std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../VP-SIGNATURE.png").is_file() {
            return;
        }
        let (pc, work) = test_pc("edit");
        // A card with a photo, already printed
        let photo = {
            let mut png = std::io::Cursor::new(Vec::new());
            image::DynamicImage::new_rgb8(40, 40).write_to(&mut png, image::ImageFormat::Png).unwrap();
            format!("data:image/png;base64,{}", BASE64.encode(png.into_inner()))
        };
        let front = generate_card(
            &pc,
            &json!({ "isFront": true, "firstName": "JUAN", "lastName": "DELA CRSU", "position": "SECURITY GUARD",
                     "hireDate": "2026-06-27", "cityOfBirth": "ANGELES CITY", "idPicture": photo }),
        )
        .unwrap();
        let control = front["controlNumber"].as_str().unwrap().to_string();
        generate_card(
            &pc,
            &json!({ "isFront": false, "controlNumber": control, "name": "Maria Cruz", "relationship": "Mother",
                     "addressLine1": "123 Rizal St", "addressLine2": "Quezon City", "contact": "0917123456",
                     "surname": "DELA CRSU", "firstName": "JUAN", "hireDate": "2026-06-27", "cityOfBirth": "ANGELES CITY" }),
        )
        .unwrap();
        pc.db.log_print(&control, "front", "", "SMART-51").unwrap();
        let old_front = find_card_file(&pc, &control, "front", "front-id.svg").unwrap();

        // The edit form gets the current values and the photo back from the saved card
        let loaded = edit::load(&pc, &control).unwrap();
        assert_eq!(loaded["front"]["lastName"], "DELA CRSU");
        assert_eq!(loaded["back"]["contact"], "0917123456");
        assert!(loaded["idPicture"].as_str().unwrap().starts_with("data:image/jpeg;base64,"));
        assert_eq!(loaded["timesPrinted"], 1);

        // Nothing changed -> nothing regenerated or logged
        assert!(edit::save(&pc, &control, &json!({})).unwrap()["changed"].as_array().unwrap().is_empty());

        // Fix the surname typo and the contact number; a hire date in the request is ignored (locked)
        let saved = edit::save(
            &pc,
            &control,
            &json!({ "lastName": "dela cruz", "contact": "09171234567", "hireDate": "2020-01-01" }),
        )
        .unwrap();
        assert_eq!(saved["needsReprint"], true);
        let changed: Vec<String> = saved["changed"].as_array().unwrap().iter()
            .map(|c| format!("{}: {} -> {}", c["field"].as_str().unwrap(), c["oldValue"].as_str().unwrap(), c["newValue"].as_str().unwrap()))
            .collect();
        assert_eq!(changed, vec!["Last name: DELA CRSU -> DELA CRUZ", "Contact number: 0917123456 -> 09171234567"]);

        // Same control number, regenerated in the corrected folder, old folder gone, photo kept
        let new_front = find_card_file(&pc, &control, "front", "front-id.svg").unwrap();
        let new_back = find_card_file(&pc, &control, "back", "back-id.svg").unwrap();
        assert!(new_front.to_string_lossy().contains(&format!("{control}_DELA CRUZ_JUAN")), "{new_front:?}");
        assert_eq!(new_front.parent(), new_back.parent());
        assert!(!old_front.exists() && !old_front.parent().unwrap().exists(), "outdated card files must be removed");
        let svg = fs::read_to_string(&new_front).unwrap();
        assert!(svg.contains("DELA CRUZ") && !svg.contains("DELA CRSU") && svg.contains(&control));
        assert!(edit::image_href(&svg, "id-pic").unwrap().starts_with("data:image/jpeg;base64,"));
        assert!(fs::read_to_string(&new_back).unwrap().contains("+63 917-123-4567"));
        assert_eq!(pc.db.latest_card_record(&control, "front").unwrap().unwrap()["hire_date"], "2026-06-27");
        assert_eq!(pc.db.card_edits(&control).unwrap().len(), 2);
        assert!(pc.db.export_candidates().unwrap().is_empty(), "a printed card is still not sendable");

        drop(pc);
        let _ = fs::remove_dir_all(work);
    }

    #[test]
    fn a_card_sent_to_another_pc_cannot_be_edited_here() {
        let (pc, work) = test_pc("edit-sent");
        let record = IdRecord {
            first_name: "ANA",
            last_name: "REYES",
            hire_date: "2026-06-27",
            city_of_birth: "ANGELES CITY",
            control_number: "260627ANG-0001",
            ..Default::default()
        };
        pc.db.save_id_record("front", &record).unwrap();
        pc.db.mark_sent_out(&["260627ANG-0001".to_string()], "").unwrap();
        assert_eq!(edit::load(&pc, "260627ANG-0001").unwrap()["sentOut"], true);
        let refused = edit::save(&pc, "260627ANG-0001", &json!({ "lastName": "REYES-CRUZ" })).unwrap_err();
        assert!(refused.contains("sent to another PC"), "{refused}");
        assert!(pc.db.card_edits("260627ANG-0001").unwrap().is_empty());
        drop(pc);
        let _ = fs::remove_dir_all(work);
    }

    #[test]
    fn import_never_overwrites_a_different_card_with_the_same_number() {
        if !std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../VP-SIGNATURE.png").is_file() {
            return;
        }
        let (pc_a, work_a) = test_pc("conflict-a");
        let (pc_b, work_b) = test_pc("conflict-b");
        let control = make_card(&pc_a, "JUAN", "DELA CRUZ");
        let exported = export_cards(&pc_a, "office-pass-2026").unwrap();
        // PC B already has someone else under that number
        pc_b.db
            .save_id_record("front", &IdRecord { first_name: "ANA", last_name: "REYES", control_number: &control, ..Default::default() })
            .unwrap();
        let data = BASE64.encode(fs::read(exported["path"].as_str().unwrap()).unwrap());
        let result = import_cards(&pc_b, &data, "office-pass-2026").unwrap();
        assert!(result["imported"].as_array().unwrap().is_empty());
        assert_eq!(result["skipped"][0]["reason"], "control number already used here by ANA REYES");
        assert_eq!(pc_b.db.card_owner(&control).unwrap().as_deref(), Some("ANA REYES"));
        // close the databases first: Windows cannot delete a file that is still open
        drop((pc_a, pc_b));
        let _ = fs::remove_dir_all(work_a);
        let _ = fs::remove_dir_all(work_b);
    }

    #[test]
    fn form_text_is_escaped_for_svg() {
        assert_eq!(xml_text("Bldg A & B <2F>"), "Bldg A &amp; B &lt;2F&gt;");
        assert_eq!(xml_text("JUAN DELA CRUZ"), "JUAN DELA CRUZ");
    }

    // Writes each template twice with sample text - browser-centred (text-anchor) and left-aligned by
    // textfit - so the two can be rendered and compared pixel by pixel.
    // Run with: OSSI_TEXTFIT_OUT=<folder> cargo test textfit_comparison -- --ignored
    #[test]
    #[ignore]
    fn textfit_comparison() {
        let out = std::path::PathBuf::from(std::env::var("OSSI_TEXTFIT_OUT").expect("OSSI_TEXTFIT_OUT"));
        let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("templates");
        let sample = [
            ("{{CONTROL_NUMBER}}", "270223CAB-9671-RH"),
            ("{{LAST_NAME}}", "DELA CRUZ JR."),
            ("{{FIRST_NAME}}", "JUAN MIGUEL D."),
            ("{{POSITION}}", "SECURITY GUARD"),
            ("{{NAME}}", "Maria S. Dela Cruz"),
            ("{{RELATIONSHIP}}", "Mother"),
            ("{{ADDRESS}}", "123 Rizal St, Brgy. Baesa,"),
            ("{{ADDRESS_LINE2}}", "Quezon City"),
            ("{{CONTACT_NO}}", "+63 917-123-4567"),
        ];
        for name in ["front-id.svg", "back-id.svg"] {
            let mut svg = fs::read_to_string(base.join(name)).unwrap();
            for (key, value) in sample {
                svg = svg.replace(key, value);
            }
            fs::write(out.join(format!("centred-{name}")), &svg).unwrap();
            fs::write(out.join(format!("textfit-{name}")), textfit::left_align_centred_text_with_installed_fonts(&svg))
                .unwrap();
        }
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
