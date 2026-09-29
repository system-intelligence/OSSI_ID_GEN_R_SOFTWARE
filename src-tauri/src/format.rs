// Card text formatting (same rules as src/phone.js and formatAddressLine1 in the Electron app)

// Philippine mobile numbers: 09XXXXXXXXX, 9XXXXXXXXX, 639XXXXXXXXX or +639XXXXXXXXX (any spaces/dashes)
// print as "+63 9XX-XXX-XXXX"; anything else (e.g. landlines) is printed as typed.
pub fn format_contact_number(value: &str) -> String {
    let digits: String = value.chars().filter(|c| c.is_ascii_digit()).collect();
    let national = digits
        .strip_prefix("63")
        .filter(|rest| rest.len() == 10)
        .or_else(|| digits.strip_prefix('0').filter(|rest| rest.len() == 10))
        .or_else(|| Some(digits.as_str()).filter(|d| d.len() == 10));
    match national {
        Some(n) if n.starts_with('9') => format!("+63 {}-{}-{}", &n[0..3], &n[3..6], &n[6..10]),
        _ => value.trim().to_string(),
    }
}

// End address line 1 with a comma when the address continues on line 2 ("St. Apple 3493," / "Pasay City")
pub fn format_address_line1(line1: &str, line2: &str) -> String {
    let first = line1.trim().trim_end_matches(|c: char| c == ',' || c.is_whitespace());
    if first.is_empty() || line2.trim().is_empty() {
        return first.to_string();
    }
    format!("{first},")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contact_numbers() {
        for typed in ["09342343453", "0934 234 3453", "0934-234-3453", "+639342343453", "639342343453", "9342343453"] {
            assert_eq!(format_contact_number(typed), "+63 934-234-3453", "{typed}");
        }
        assert_eq!(format_contact_number("09347348734854"), "09347348734854");
        assert_eq!(format_contact_number("0934234345"), "0934234345");
        assert_eq!(format_contact_number("(02) 8123-4567"), "(02) 8123-4567");
        assert_eq!(format_contact_number(" 8123-4567 "), "8123-4567");
        assert_eq!(format_contact_number(""), "");
    }

    #[test]
    fn address_comma() {
        assert_eq!(format_address_line1("St. Apple 3493", "Pasay City"), "St. Apple 3493,");
        assert_eq!(format_address_line1("St. Apple 3493,", "Pasay City"), "St. Apple 3493,");
        assert_eq!(format_address_line1("St. Apple 3493 , ", "Pasay City"), "St. Apple 3493,");
        assert_eq!(format_address_line1("St. Apple 3493", ""), "St. Apple 3493");
        assert_eq!(format_address_line1("", "Pasay City"), "");
    }
}
