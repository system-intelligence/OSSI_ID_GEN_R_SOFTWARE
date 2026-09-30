// SQLite access - same schema, migrations and queries as src/db.js in the Electron app,
// so both versions can open the same id-generator.db.
use rusqlite::{params, types::ValueRef, Connection, OptionalExtension};
use serde_json::{json, Map, Value};
use std::path::Path;
use std::sync::Mutex;

use crate::cities::CITIES;

pub struct Db(Mutex<Connection>);

// Fields of one id_records row; empty strings are stored as NULL (like `value || null` in db.js)
#[derive(Default)]
pub struct IdRecord<'a> {
    pub first_name: &'a str,
    pub last_name: &'a str,
    pub middle_initial: &'a str,
    pub suffix: &'a str,
    pub position: &'a str,
    pub employee_name: &'a str,
    pub hire_date: &'a str,
    pub city_of_birth: &'a str,
    pub city_code: &'a str,
    pub address1: &'a str,
    pub address2: &'a str,
    pub relationship: &'a str,
    pub contact: &'a str,
    pub is_rehire: bool,
    pub control_number: &'a str,
    // Generated SVG, relative to the ID/ folder, e.g. 260627ANG-0950_DELA CRUZ_JUAN/front-id.svg
    pub file_path: &'a str,
}

fn or_null(value: &str) -> Option<&str> {
    if value.is_empty() { None } else { Some(value) }
}

// Same format as JavaScript's new Date().toISOString(), e.g. 2026-09-29T03:04:05.123Z
fn now_iso() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
}

// ALTER TABLE ... ADD COLUMN, ignoring "duplicate column" when the migration already ran
fn add_column(conn: &Connection, sql: &str) {
    if let Err(err) = conn.execute(sql, []) {
        let msg = err.to_string();
        if !msg.contains("duplicate column") && !msg.contains("already exists") {
            eprintln!("Migration error: {msg}");
        }
    }
}

impl Db {
    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        let conn = Connection::open(path)?;
        let db = Db(Mutex::new(conn));
        db.init()?;
        Ok(db)
    }

    fn conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.0.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn init(&self) -> rusqlite::Result<()> {
        let mut conn = self.conn();

        conn.execute(
            "CREATE TABLE IF NOT EXISTS cities (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT UNIQUE NOT NULL,
                code TEXT UNIQUE NOT NULL
            )",
            [],
        )?;
        let tx = conn.transaction()?;
        {
            let mut stmt = tx.prepare("INSERT OR IGNORE INTO cities (name, code) VALUES (?, ?)")?;
            for (name, code) in CITIES {
                if let Err(err) = stmt.execute(params![name, code]) {
                    eprintln!("City insert error: {err}");
                }
            }
        }
        tx.commit()?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS control_numbers (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                prefix TEXT NOT NULL,
                sequence INTEGER NOT NULL,
                control_number TEXT NOT NULL,
                is_rehire INTEGER DEFAULT 0,
                created_at TEXT NOT NULL
            )",
            [],
        )?;
        conn.execute("CREATE UNIQUE INDEX IF NOT EXISTS idx_control_number ON control_numbers(control_number)", [])?;
        for sql in [
            "ALTER TABLE control_numbers ADD COLUMN employee_name TEXT",
            "ALTER TABLE control_numbers ADD COLUMN hire_date TEXT",
            "ALTER TABLE control_numbers ADD COLUMN city_of_birth TEXT",
            "ALTER TABLE control_numbers ADD COLUMN city_code TEXT",
            "ALTER TABLE control_numbers ADD COLUMN id_number TEXT",
        ] {
            add_column(&conn, sql);
        }

        conn.execute(
            "CREATE TABLE IF NOT EXISTS id_records (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                type TEXT NOT NULL,
                id_number TEXT,
                first_name TEXT,
                last_name TEXT,
                middle_initial TEXT,
                suffix TEXT,
                position TEXT,
                employee_name TEXT,
                hire_date TEXT,
                city_of_birth TEXT,
                city_code TEXT,
                address1 TEXT,
                address2 TEXT,
                contact TEXT,
                is_rehire INTEGER DEFAULT 0,
                control_number TEXT,
                created_at TEXT NOT NULL,
                relationship TEXT
            )",
            [],
        )?;
        add_column(&conn, "ALTER TABLE id_records ADD COLUMN relationship TEXT");
        add_column(&conn, "ALTER TABLE id_records ADD COLUMN file_path TEXT");

        // Every time a card side is sent to the printer. "Sent", not "printed": the app cannot see whether
        // the operator cancelled the print dialog or the printer jammed.
        conn.execute(
            "CREATE TABLE IF NOT EXISTS print_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                control_number TEXT NOT NULL,
                side TEXT NOT NULL,
                is_reprint INTEGER NOT NULL DEFAULT 0,
                reason TEXT,
                printer TEXT,
                printed_at TEXT NOT NULL
            )",
            [],
        )?;
        conn.execute("CREATE INDEX IF NOT EXISTS idx_print_log_control ON print_log(control_number)", [])?;
        Ok(())
    }

    pub fn get_all_cities(&self) -> rusqlite::Result<Vec<Value>> {
        let conn = self.conn();
        let mut stmt = conn.prepare("SELECT name, code FROM cities ORDER BY name ASC")?;
        let rows = stmt.query_map([], |row| {
            Ok(json!({ "name": row.get::<_, String>(0)?, "code": row.get::<_, String>(1)? }))
        })?;
        rows.collect()
    }

    pub fn get_city_code(&self, city_name: &str) -> rusqlite::Result<Option<String>> {
        if city_name.is_empty() {
            return Ok(None);
        }
        self.conn()
            .query_row("SELECT code FROM cities WHERE name = ?", [city_name.to_uppercase()], |row| row.get(0))
            .optional()
    }

    // Random 4-digit sequence not yet used for this prefix, e.g. 260627ANG-0950 (or ...-0950-RH)
    pub fn generate_control_number(&self, prefix: &str, is_rehire: bool) -> Result<String, String> {
        let conn = self.conn();
        for _ in 0..100 {
            let seq = fastrand::u32(0..10000);
            let taken = conn
                .query_row(
                    "SELECT id FROM control_numbers WHERE prefix = ? AND sequence = ?",
                    params![prefix, seq],
                    |_| Ok(()),
                )
                .optional()
                .map_err(|e| e.to_string())?
                .is_some();
            if !taken {
                let suffix = if is_rehire { "-RH" } else { "" };
                return Ok(format!("{prefix}-{seq:04}{suffix}"));
            }
        }
        Err("Unable to generate unique control number after multiple attempts".into())
    }

    pub fn reserve_control_number(&self, prefix: &str, sequence: u32, is_rehire: bool) -> rusqlite::Result<String> {
        let suffix = if is_rehire { "-RH" } else { "" };
        let control_number = format!("{prefix}-{sequence:04}{suffix}");
        self.conn().execute(
            "INSERT OR IGNORE INTO control_numbers (prefix, sequence, control_number, is_rehire, created_at) VALUES (?, ?, ?, ?, ?)",
            params![prefix, sequence, control_number, is_rehire as i32, now_iso()],
        )?;
        Ok(control_number)
    }

    pub fn save_id_record(&self, record_type: &str, r: &IdRecord) -> rusqlite::Result<()> {
        self.conn().execute(
            "INSERT INTO id_records
             (type, id_number, first_name, last_name, middle_initial, suffix, position, employee_name, hire_date, city_of_birth, city_code, address1, address2, relationship, contact, is_rehire, control_number, created_at, file_path)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                or_null(record_type).unwrap_or("front"),
                None::<&str>,
                or_null(r.first_name),
                or_null(r.last_name),
                or_null(r.middle_initial),
                or_null(r.suffix),
                or_null(r.position),
                or_null(r.employee_name),
                or_null(r.hire_date),
                or_null(r.city_of_birth),
                or_null(r.city_code),
                or_null(r.address1),
                or_null(r.address2),
                or_null(r.relationship),
                or_null(r.contact),
                r.is_rehire as i32,
                or_null(r.control_number),
                now_iso(),
                or_null(r.file_path),
            ],
        )?;
        Ok(())
    }

    // Most recently saved file of one side ("front" / "back") of a card, relative to the ID/ folder
    pub fn latest_card_file(&self, control_number: &str, record_type: &str) -> rusqlite::Result<Option<String>> {
        self.conn()
            .query_row(
                "SELECT file_path FROM id_records
                 WHERE control_number = ? AND type = ? AND file_path IS NOT NULL
                 ORDER BY created_at DESC LIMIT 1",
                params![control_number, record_type],
                |row| row.get(0),
            )
            .optional()
    }

    // Every column of every record, newest first, as JSON objects keyed by column name
    pub fn get_all_id_records(&self) -> rusqlite::Result<Vec<Value>> {
        let conn = self.conn();
        let mut stmt = conn.prepare("SELECT * FROM id_records ORDER BY created_at DESC")?;
        let names: Vec<String> = stmt.column_names().iter().map(|n| n.to_string()).collect();
        let rows = stmt.query_map([], |row| {
            let mut obj = Map::new();
            for (i, name) in names.iter().enumerate() {
                let value = match row.get_ref(i)? {
                    ValueRef::Null => Value::Null,
                    ValueRef::Integer(n) => json!(n),
                    ValueRef::Real(f) => json!(f),
                    ValueRef::Text(t) => Value::String(String::from_utf8_lossy(t).into_owned()),
                    ValueRef::Blob(_) => Value::Null,
                };
                obj.insert(name.clone(), value);
            }
            Ok(Value::Object(obj))
        })?;
        rows.collect()
    }

    // Records one card side sent to the printer; it is a reprint when that side was sent before
    pub fn log_print(&self, control_number: &str, side: &str, reason: &str, printer: &str) -> rusqlite::Result<Value> {
        let conn = self.conn();
        let previous: i64 = conn.query_row(
            "SELECT COUNT(*) FROM print_log WHERE control_number = ? AND side = ?",
            params![control_number, side],
            |row| row.get(0),
        )?;
        let printed_at = now_iso();
        conn.execute(
            "INSERT INTO print_log (control_number, side, is_reprint, reason, printer, printed_at) VALUES (?, ?, ?, ?, ?, ?)",
            params![control_number, side, (previous > 0) as i32, or_null(reason), or_null(printer), printed_at],
        )?;
        Ok(json!({ "isReprint": previous > 0, "printedAt": printed_at, "count": previous + 1 }))
    }

    // Print history of one card, newest first
    pub fn print_history(&self, control_number: &str) -> rusqlite::Result<Vec<Value>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT side, is_reprint, reason, printer, printed_at FROM print_log
             WHERE control_number = ? ORDER BY printed_at DESC, id DESC",
        )?;
        let rows = stmt.query_map([control_number], |row| {
            Ok(json!({
                "side": row.get::<_, String>(0)?,
                "isReprint": row.get::<_, i64>(1)? != 0,
                "reason": row.get::<_, Option<String>>(2)?,
                "printer": row.get::<_, Option<String>>(3)?,
                "printedAt": row.get::<_, String>(4)?,
            }))
        })?;
        rows.collect()
    }

    // The whole print log, newest first, with the employee's name from the card's latest front record
    pub fn print_log_entries(&self) -> rusqlite::Result<Vec<Value>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT p.control_number, p.side, p.is_reprint, p.reason, p.printer, p.printed_at,
                    r.first_name, r.middle_initial, r.last_name, r.suffix
             FROM print_log p
             LEFT JOIN id_records r ON r.id = (
                 SELECT id FROM id_records
                 WHERE control_number = p.control_number AND type = 'front'
                 ORDER BY created_at DESC, id DESC LIMIT 1
             )
             ORDER BY p.printed_at DESC, p.id DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            let name = (6..=9)
                .filter_map(|i| row.get::<_, Option<String>>(i).ok().flatten())
                .map(|part| part.trim().to_string())
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>()
                .join(" ");
            Ok(json!({
                "controlNumber": row.get::<_, String>(0)?,
                "side": row.get::<_, String>(1)?,
                "isReprint": row.get::<_, i64>(2)? != 0,
                "reason": row.get::<_, Option<String>>(3)?,
                "printer": row.get::<_, Option<String>>(4)?,
                "printedAt": row.get::<_, String>(5)?,
                "name": name,
            }))
        })?;
        rows.collect()
    }

    // Per control number: how many times each side was sent to the printer and when last, for the Records table
    pub fn print_summary(&self) -> rusqlite::Result<Map<String, Value>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT control_number,
                    SUM(side = 'front'), SUM(side = 'back'), MAX(printed_at)
             FROM print_log GROUP BY control_number",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                json!({
                    "front": row.get::<_, i64>(1)?,
                    "back": row.get::<_, i64>(2)?,
                    "lastPrintedAt": row.get::<_, String>(3)?,
                }),
            ))
        })?;
        rows.collect()
    }

    pub fn reset_database(&self) -> rusqlite::Result<()> {
        let conn = self.conn();
        conn.execute("DELETE FROM id_records", [])?;
        conn.execute("DELETE FROM control_numbers", [])?;
        conn.execute("DELETE FROM print_log", [])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn memory_db() -> Db {
        Db::open(Path::new(":memory:")).unwrap()
    }

    #[test]
    fn first_print_is_not_a_reprint_but_the_second_is() {
        let db = memory_db();
        let first = db.log_print("270223CAB-9671-RH", "front", "", "SMART-51").unwrap();
        assert_eq!(first["isReprint"], false);
        assert_eq!(first["count"], 1);

        // The back is a different side, so its first print is not a reprint either
        assert_eq!(db.log_print("270223CAB-9671-RH", "back", "", "SMART-51").unwrap()["isReprint"], false);

        let again = db.log_print("270223CAB-9671-RH", "front", "Lost", "SMART-51").unwrap();
        assert_eq!(again["isReprint"], true);
        assert_eq!(again["count"], 2);
    }

    #[test]
    fn history_and_summary() {
        let db = memory_db();
        db.log_print("260627ANG-0950", "front", "", "").unwrap();
        db.log_print("260627ANG-0950", "front", "Damaged", "SMART-51").unwrap();
        db.log_print("260627ANG-0950", "back", "", "").unwrap();
        db.log_print("251231QC-0001", "front", "", "").unwrap();

        let history = db.print_history("260627ANG-0950").unwrap();
        assert_eq!(history.len(), 3);
        let reprint = history.iter().find(|h| h["isReprint"] == true).unwrap();
        assert_eq!(reprint["reason"], "Damaged");
        assert_eq!(reprint["printer"], "SMART-51");
        assert!(history.iter().filter(|h| h["isReprint"] == false).all(|h| h["reason"].is_null()));

        let summary = db.print_summary().unwrap();
        assert_eq!(summary["260627ANG-0950"]["front"], 2);
        assert_eq!(summary["260627ANG-0950"]["back"], 1);
        assert_eq!(summary["251231QC-0001"]["front"], 1);
        assert_eq!(summary["251231QC-0001"]["back"], 0);
    }

    #[test]
    fn full_log_includes_employee_names() {
        let db = memory_db();
        let record = IdRecord {
            first_name: "JIRRUM",
            middle_initial: "D.",
            last_name: "EDICA",
            control_number: "270223CAB-9671-RH",
            ..Default::default()
        };
        db.save_id_record("front", &record).unwrap();
        db.log_print("270223CAB-9671-RH", "front", "", "SMART-51").unwrap();
        db.log_print("270223CAB-9671-RH", "front", "Lost", "SMART-51").unwrap();
        db.log_print("999999XXX-0000", "back", "", "").unwrap(); // no record for this number

        let log = db.print_log_entries().unwrap();
        assert_eq!(log.len(), 3);
        let named: Vec<_> = log.iter().filter(|e| e["controlNumber"] == "270223CAB-9671-RH").collect();
        assert!(named.iter().all(|e| e["name"] == "JIRRUM D. EDICA"));
        assert!(named.iter().any(|e| e["isReprint"] == true && e["reason"] == "Lost"));
        assert_eq!(log.iter().find(|e| e["controlNumber"] == "999999XXX-0000").unwrap()["name"], "");
    }

    #[test]
    fn reset_clears_the_print_log() {
        let db = memory_db();
        db.log_print("260627ANG-0950", "front", "", "").unwrap();
        db.reset_database().unwrap();
        assert!(db.print_summary().unwrap().is_empty());
        assert_eq!(db.log_print("260627ANG-0950", "front", "", "").unwrap()["isReprint"], false);
    }
}
