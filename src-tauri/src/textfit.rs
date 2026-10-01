// Centred card text, made exact for CorelDRAW.
//
// The templates centre lines with text-anchor="middle". Browsers centre them with their own text
// measurement, but CorelDRAW X7 measures differently when importing, so centred lines land slightly off.
// Here every centred <text> is measured with the actual installed font (shaped with rustybuzz, the Rust
// port of HarfBuzz that Chrome/WebView2 use) and rewritten as a plain left-anchored line starting at
// x - width/2. Corel then has nothing to estimate, and the text stays editable.
// When a font cannot be found the line is left as it was, so nothing is ever worse than before.
use rustybuzz::ttf_parser;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

#[derive(Clone)]
struct FontFile {
    path: PathBuf,
    index: u32,
}

// Font folders: the system one, and the per-user one (fonts installed "for me only", e.g. Windsor BT)
fn font_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(windir) = std::env::var("WINDIR") {
        dirs.push(PathBuf::from(windir).join("Fonts"));
    }
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        dirs.push(PathBuf::from(local).join("Microsoft").join("Windows").join("Fonts"));
    }
    dirs
}

fn normalize_family(name: &str) -> String {
    name.trim().trim_matches(|c| c == '\'' || c == '"').to_lowercase()
}

// Family name as CSS/SVG uses it: the typographic family (name ID 16) if present, else the family (ID 1)
fn family_names(face: &ttf_parser::Face) -> Vec<String> {
    let mut names = Vec::new();
    for wanted in [ttf_parser::name_id::TYPOGRAPHIC_FAMILY, ttf_parser::name_id::FAMILY] {
        for name in face.names() {
            if name.name_id == wanted && name.is_unicode() {
                if let Some(text) = name.to_string() {
                    names.push(normalize_family(&text));
                }
            }
        }
    }
    names.dedup();
    names
}

// (family, bold) -> font file; built once by reading the name table of every installed font
fn font_index() -> &'static HashMap<(String, bool), FontFile> {
    static INDEX: OnceLock<HashMap<(String, bool), FontFile>> = OnceLock::new();
    INDEX.get_or_init(|| {
        let mut index = HashMap::new();
        for dir in font_dirs() {
            let Ok(entries) = std::fs::read_dir(&dir) else { continue };
            for entry in entries.flatten() {
                let path = entry.path();
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
                if !matches!(ext.as_str(), "ttf" | "otf" | "ttc") {
                    continue;
                }
                let Ok(data) = std::fs::read(&path) else { continue };
                let count = ttf_parser::fonts_in_collection(&data).unwrap_or(1);
                for i in 0..count {
                    let Ok(face) = ttf_parser::Face::parse(&data, i) else { continue };
                    if face.is_italic() || face.is_oblique() {
                        continue;
                    }
                    let bold = face.is_bold() || face.weight().to_number() >= 600;
                    for family in family_names(&face) {
                        index.entry((family, bold)).or_insert_with(|| FontFile { path: path.clone(), index: i });
                    }
                }
            }
        }
        index
    })
}

fn font_bytes(file: &FontFile) -> Option<std::sync::Arc<Vec<u8>>> {
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, std::sync::Arc<Vec<u8>>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let mut cache = cache.lock().unwrap_or_else(|p| p.into_inner());
    if let Some(data) = cache.get(&file.path) {
        return Some(data.clone());
    }
    let data = std::sync::Arc::new(std::fs::read(&file.path).ok()?);
    cache.insert(file.path.clone(), data.clone());
    Some(data)
}

// Width of `text` in user units, as Chrome lays it out: HarfBuzz shaping with the default features
// (kerning, ligatures), plus letter-spacing after every character
pub fn measure(family_list: &str, bold: bool, size: f64, letter_spacing: f64, text: &str) -> Option<f64> {
    // First family in the list that is installed, like CSS font fallback
    let file = family_list
        .split(',')
        .map(normalize_family)
        .find_map(|family| font_index().get(&(family, bold)).cloned())?;
    let data = font_bytes(&file)?;
    let face = rustybuzz::Face::from_slice(&data, file.index)?;
    let mut buffer = rustybuzz::UnicodeBuffer::new();
    buffer.push_str(text);
    let glyphs = rustybuzz::shape(&face, &[], buffer);
    let advance: i64 = glyphs.glyph_positions().iter().map(|p| p.x_advance as i64).sum();
    let units_per_em = face.units_per_em() as f64;
    Some(advance as f64 * size / units_per_em + letter_spacing * text.chars().count() as f64)
}

fn attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let key = format!(" {name}=\"");
    let start = tag.find(&key)? + key.len();
    let end = tag[start..].find('"')? + start;
    Some(&tag[start..end])
}

fn set_attr(tag: &str, name: &str, value: &str) -> String {
    let key = format!(" {name}=\"");
    match tag.find(&key) {
        Some(pos) => {
            let start = pos + key.len();
            let end = tag[start..].find('"').map(|e| e + start).unwrap_or(start);
            format!("{}{}{}", &tag[..start], value, &tag[end..])
        }
        None => tag.to_string(),
    }
}

// Removes ` name="..."` together with any whitespace (including a line break) in front of it
fn remove_attr(tag: &str, name: &str) -> String {
    let key = format!("{name}=\"");
    let Some(pos) = tag.find(&key) else { return tag.to_string() };
    let end = tag[pos + key.len()..].find('"').map(|e| e + pos + key.len() + 1).unwrap_or(tag.len());
    let start = tag[..pos].trim_end().len();
    format!("{}{}", &tag[..start], &tag[end..])
}

fn decode_entities(text: &str) -> String {
    text.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'").replace("&amp;", "&")
}

// SVG text (xml:space default) drops leading/trailing spaces and collapses runs of spaces. Browsers do this
// on display, Corel may not, so the text is written out already collapsed.
fn collapse_spaces(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

// Rewrites every simple centred <text> (no nested elements) in `svg` with an exact left x.
// `measure_fn(family, bold, size, letter_spacing, text)` returns the width, or None to leave the line as is.
pub fn left_align_centred_text<F>(svg: &str, measure_fn: F) -> String
where
    F: Fn(&str, bool, f64, f64, &str) -> Option<f64>,
{
    let mut out = String::with_capacity(svg.len());
    let mut rest = svg;
    while let Some(start) = rest.find("<text") {
        out.push_str(&rest[..start]);
        let after = &rest[start..];
        let (Some(tag_end), Some(close)) = (after.find('>'), after.find("</text>")) else {
            out.push_str(after);
            return out;
        };
        let tag = &after[..tag_end + 1];
        let content = &after[tag_end + 1..close];
        let element_end = close + "</text>".len();
        let rewritten = (|| {
            if attr(tag, "text-anchor") != Some("middle") || content.contains('<') || tag_end > close {
                return None;
            }
            let x: f64 = attr(tag, "x")?.trim().parse().ok()?;
            let size: f64 = attr(tag, "font-size")?.trim().parse().ok()?;
            let family = attr(tag, "font-family")?;
            let bold = matches!(attr(tag, "font-weight"), Some("bold" | "bolder" | "600" | "700" | "800" | "900"));
            let spacing: f64 = attr(tag, "letter-spacing").and_then(|s| s.trim().parse().ok()).unwrap_or(0.0);
            let text = collapse_spaces(content);
            if text.is_empty() {
                return None;
            }
            let width = measure_fn(family, bold, size, spacing, &decode_entities(&text))?;
            let new_tag = remove_attr(&set_attr(tag, "x", &format!("{:.2}", x - width / 2.0)), "text-anchor");
            Some(format!("{new_tag}{text}</text>"))
        })();
        match rewritten {
            Some(element) => out.push_str(&element),
            None => out.push_str(&after[..element_end]),
        }
        rest = &after[element_end..];
    }
    out.push_str(rest);
    out
}

// The real thing: measured with the fonts installed on this PC
pub fn left_align_centred_text_with_installed_fonts(svg: &str) -> String {
    left_align_centred_text(svg, measure)
}

#[cfg(test)]
mod tests {
    use super::*;

    // 10 units per character, so widths are easy to check
    fn fake(_: &str, _: bool, _: f64, spacing: f64, text: &str) -> Option<f64> {
        Some(text.chars().count() as f64 * (10.0 + spacing))
    }

    #[test]
    fn centred_text_becomes_left_anchored_at_the_exact_start() {
        let svg = r#"<svg><text x="1062" y="2713.8"
        font-family="Arial"
        font-size="120"
        text-anchor="middle">if found</text></svg>"#;
        let out = left_align_centred_text(svg, fake);
        // "if found" = 8 characters = 80 wide, so it starts 40 left of the centre
        assert!(out.contains(r#"<text x="1022.00" y="2713.8""#), "{out}");
        assert!(!out.contains("text-anchor"), "{out}");
        assert!(out.contains(">if found</text>"), "{out}");
    }

    #[test]
    fn spaces_are_collapsed_and_entities_measured_as_characters() {
        let svg = r#"<text x="100" font-family="Arial" font-size="10" text-anchor="middle">  A &amp;   B </text>"#;
        let out = left_align_centred_text(svg, fake);
        // measured text is "A & B" = 5 characters = 50 wide
        assert_eq!(out, r#"<text x="75.00" font-family="Arial" font-size="10">A &amp; B</text>"#);
    }

    #[test]
    fn letter_spacing_and_bold_are_passed_to_the_measurement() {
        let seen = std::cell::RefCell::new(Vec::new());
        let svg = r#"<text x="0" font-family="Arial" font-weight="bold" font-size="85" letter-spacing="3" text-anchor="middle">AB</text>"#;
        left_align_centred_text(svg, |family, bold, size, spacing, text| {
            seen.borrow_mut().push((family.to_string(), bold, size, spacing, text.to_string()));
            Some(0.0)
        });
        assert_eq!(seen.into_inner(), vec![("Arial".to_string(), true, 85.0, 3.0, "AB".to_string())]);
    }

    #[test]
    fn other_text_is_left_untouched() {
        let svg = r#"<text x="130" y="185" font-family="Arial" font-size="97">NAME:</text><text x="5" font-family="Nope" font-size="9" text-anchor="middle">X</text>"#;
        // first: not centred; second: font not found (measure returns None)
        assert_eq!(left_align_centred_text(svg, |f, _, _, _, _| if f == "Nope" { None } else { Some(1.0) }), svg);
    }

    #[test]
    fn installed_arial_is_measured_like_a_browser() {
        // Skipped on machines without Arial (e.g. a Linux CI); on Windows it is always there
        let Some(width) = measure("Arial", false, 1000.0, 0.0, "H") else { return };
        // Arial "H" advance is 1479/2048 em
        assert!((width - 1000.0 * 1479.0 / 2048.0).abs() < 0.01, "{width}");
    }
}
