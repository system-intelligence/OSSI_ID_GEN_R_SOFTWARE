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

    pub fn reset_database(&self) -> rusqlite::Result<()> {
        let conn = self.conn();
        conn.execute("DELETE FROM id_records", [])?;
        conn.execute("DELETE FROM control_numbers", [])?;
        Ok(())
    }
}
