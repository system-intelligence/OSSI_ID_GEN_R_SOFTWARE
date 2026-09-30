// Is the ID card printer (IDP SMART-51) installed in Windows, and is it plugged in?
// Windows' own printer status says "Normal" even for an unplugged USB printer, so "connected" is taken from
// the device list instead: a USB printer's print queue device is only present while the printer is attached.
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Deserialize, Default)]
pub struct WindowsPrinters {
    #[serde(default)]
    printers: Vec<PrinterInfo>,
    // Names of the print queue devices currently present (attached)
    #[serde(default)]
    present: Vec<String>,
    // Installed printer drivers; a USB driver package can be installed before the printer was ever plugged in,
    // in which case Windows only creates the printer on first connection
    #[serde(default)]
    drivers: Vec<String>,
}

#[derive(Deserialize, Clone)]
struct PrinterInfo {
    name: String,
    #[serde(default)]
    driver: String,
    #[serde(default)]
    status: String,
}

// "SMART-51", "Smart 51S", "IDP SMART51 Card Printer"... -> contains "smart51"
fn is_smart51(text: &str) -> bool {
    text.chars().filter(char::is_ascii_alphanumeric).collect::<String>().to_lowercase().contains("smart51")
}

fn is_card_printer(printer: &PrinterInfo) -> bool {
    is_smart51(&printer.name) || is_smart51(&printer.driver)
}

// state: "ready" | "attention" (connected but Windows reports a problem) | "disconnected"
//      | "never_connected" (driver installed, printer not plugged in yet) | "not_installed"
pub fn summarize(info: &WindowsPrinters) -> Value {
    let card_printers: Vec<&PrinterInfo> = info.printers.iter().filter(|p| is_card_printer(p)).collect();
    let Some(first) = card_printers.first() else {
        return match info.drivers.iter().find(|d| is_smart51(d)) {
            Some(driver) => json!({ "state": "never_connected", "driver": driver }),
            None => json!({ "state": "not_installed" }),
        };
    };
    // With several copies of the driver installed, report the one that is plugged in
    let connected = card_printers.iter().find(|p| info.present.iter().any(|name| name.eq_ignore_ascii_case(&p.name)));
    match connected {
        None => json!({ "state": "disconnected", "printer": first.name }),
        Some(p) if p.status.is_empty() || p.status.eq_ignore_ascii_case("normal") => {
            json!({ "state": "ready", "printer": p.name })
        }
        Some(p) => json!({ "state": "attention", "printer": p.name, "detail": p.status }),
    }
}

#[cfg(windows)]
fn query_windows() -> Result<WindowsPrinters, String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000; // no console window flashing up

    let script = r#"
$ErrorActionPreference = 'SilentlyContinue'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
$printers = @(Get-Printer | ForEach-Object { [pscustomobject]@{ name = $_.Name; driver = $_.DriverName; status = "$($_.PrinterStatus)" } })
$present = @(Get-PnpDevice -PresentOnly -Class PrintQueue | ForEach-Object { $_.FriendlyName })
$drivers = @(Get-PrinterDriver | ForEach-Object { $_.Name })
[pscustomobject]@{ printers = $printers; present = $present; drivers = $drivers } | ConvertTo-Json -Depth 3 -Compress
"#;
    let output = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", script])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("Could not ask Windows for printers: {e}"))?;
    let text = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str(text.trim()).map_err(|e| format!("Unexpected printer list from Windows: {e}"))
}

#[cfg(not(windows))]
fn query_windows() -> Result<WindowsPrinters, String> {
    Err("Printer detection is only available on Windows".into())
}

pub fn status() -> Value {
    match query_windows() {
        Ok(info) => summarize(&info),
        Err(error) => json!({ "state": "unknown", "error": error }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(printers: &[(&str, &str, &str)], present: &[&str]) -> WindowsPrinters {
        WindowsPrinters {
            printers: printers
                .iter()
                .map(|(name, driver, status)| PrinterInfo { name: name.to_string(), driver: driver.to_string(), status: status.to_string() })
                .collect(),
            present: present.iter().map(|s| s.to_string()).collect(),
            drivers: Vec::new(),
        }
    }

    #[test]
    fn no_card_printer_installed() {
        let windows = info(&[("EPSON L3210 Series", "EPSON L3210 Series", "Normal")], &["EPSON L3210 Series"]);
        assert_eq!(summarize(&windows)["state"], "not_installed");
    }

    #[test]
    fn driver_installed_but_printer_never_plugged_in() {
        let mut windows = info(&[("EPSON L3210 Series", "EPSON L3210 Series", "Normal")], &[]);
        windows.drivers = vec!["EPSON L3210 Series".into(), "IDP SMART-51 Card Printer".into()];
        let status = summarize(&windows);
        assert_eq!(status["state"], "never_connected");
        assert_eq!(status["driver"], "IDP SMART-51 Card Printer");
    }

    #[test]
    fn installed_but_unplugged() {
        let windows = info(&[("SMART-51 Card Printer", "IDP SMART-51", "Normal")], &["Fax"]);
        assert_eq!(summarize(&windows)["state"], "disconnected");
    }

    #[test]
    fn plugged_in_and_ready() {
        let windows = info(&[("IDP Smart 51S", "IDP Card Printer", "Normal")], &["idp smart 51s"]);
        let status = summarize(&windows);
        assert_eq!(status["state"], "ready");
        assert_eq!(status["printer"], "IDP Smart 51S");
    }

    #[test]
    fn plugged_in_copy_is_preferred_and_problems_reported() {
        let windows = info(
            &[("SMART-51", "SMART-51 Driver", "Normal"), ("SMART-51 (Copy 1)", "SMART-51 Driver", "PaperOut")],
            &["SMART-51 (Copy 1)"],
        );
        let status = summarize(&windows);
        assert_eq!(status["state"], "attention");
        assert_eq!(status["printer"], "SMART-51 (Copy 1)");
        assert_eq!(status["detail"], "PaperOut");
    }
}
