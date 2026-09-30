// QR code for the back ID, drawn as a single SVG <path> (horizontal runs of dark modules merged into rectangles)
// so it stays a crisp vector shape in the browser, in CorelDRAW and on the card printer.
use qrcode::{Color, EcLevel, QrCode};

// Where the QR goes on templates/back-id.svg (card units are 1/1000 inch): a 0.52 in square
pub const QR_X: f64 = 225.5;
pub const QR_Y: f64 = 1888.67;
pub const QR_SIZE: f64 = 520.0;

// `size` is the width of the modules only; the white card around the code serves as the quiet zone.
// Error correction M (15%) keeps control numbers like 270223CAB-9671-RH at version 1 (21 x 21), the largest
// and easiest to scan modules in this box, while still tolerating light scratches.
pub fn qr_svg_path(data: &str, x: f64, y: f64, size: f64) -> Result<String, String> {
    let code = QrCode::with_error_correction_level(data.as_bytes(), EcLevel::M).map_err(|e| format!("QR code: {e}"))?;
    let modules = code.width();
    let colors = code.to_colors();
    let cell = size / modules as f64;

    let mut d = String::new();
    for row in 0..modules {
        let mut col = 0;
        while col < modules {
            if colors[row * modules + col] != Color::Dark {
                col += 1;
                continue;
            }
            let start = col;
            while col < modules && colors[row * modules + col] == Color::Dark {
                col += 1;
            }
            let (left, top, width) = (x + start as f64 * cell, y + row as f64 * cell, (col - start) as f64 * cell);
            d.push_str(&format!("M{left:.2} {top:.2}h{width:.2}v{cell:.2}h-{width:.2}z"));
        }
    }
    Ok(format!(r##"<path fill="#000000" shape-rendering="crispEdges" d="{d}"/>"##))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_numbers_fit_version_1() {
        for control in ["260627ANG-0950", "270223CAB-9671-RH"] {
            let code = QrCode::with_error_correction_level(control.as_bytes(), EcLevel::M).unwrap();
            assert_eq!(code.width(), 21, "{control}");
        }
    }

    #[test]
    fn path_fills_the_box_and_starts_with_the_finder_pattern() {
        let path = qr_svg_path("270223CAB-9671-RH", QR_X, QR_Y, QR_SIZE).unwrap();
        // Top-left finder pattern: the first run is 7 modules wide at the box corner
        assert!(path.contains(r#"d="M225.50 1888.67h173.33v24.76h-173.33z"#), "{path}");
        // Top-right finder pattern ends exactly at the right edge of the box (225.5 + 520)
        assert!(path.contains("M572.17 1888.67h173.33"), "{path}");
    }

    #[test]
    fn different_control_numbers_give_different_codes() {
        let a = qr_svg_path("260627ANG-0950", QR_X, QR_Y, QR_SIZE).unwrap();
        let b = qr_svg_path("260627ANG-0951", QR_X, QR_Y, QR_SIZE).unwrap();
        assert_ne!(a, b);
    }
}
