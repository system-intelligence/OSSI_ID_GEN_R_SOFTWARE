// Province of birth -> 3-letter code used in new control numbers (e.g. 260627PAM-0950).
// Codes are the official ISO 3166-2:PH province codes (82 provinces, ISO newsletter of 2023-11-23:
// Maguindanao del Norte / del Sur). Metro Manila is a region with no ISO letter code (PH-00), so it uses NCR.
// Note: ISO keeps codes when a province is renamed - Cotabato is NCO (North Cotabato), Davao de Oro is COM
// (Compostela Valley), Samar is WSA (Western Samar).
pub const PROVINCES: &[(&str, &str)] = &[
    ("ABRA", "ABR"),
    ("AGUSAN DEL NORTE", "AGN"),
    ("AGUSAN DEL SUR", "AGS"),
    ("AKLAN", "AKL"),
    ("ALBAY", "ALB"),
    ("ANTIQUE", "ANT"),
    ("APAYAO", "APA"),
    ("AURORA", "AUR"),
    ("BASILAN", "BAS"),
    ("BATAAN", "BAN"),
    ("BATANES", "BTN"),
    ("BATANGAS", "BTG"),
    ("BENGUET", "BEN"),
    ("BILIRAN", "BIL"),
    ("BOHOL", "BOH"),
    ("BUKIDNON", "BUK"),
    ("BULACAN", "BUL"),
    ("CAGAYAN", "CAG"),
    ("CAMARINES NORTE", "CAN"),
    ("CAMARINES SUR", "CAS"),
    ("CAMIGUIN", "CAM"),
    ("CAPIZ", "CAP"),
    ("CATANDUANES", "CAT"),
    ("CAVITE", "CAV"),
    ("CEBU", "CEB"),
    ("COTABATO", "NCO"),
    ("DAVAO DE ORO", "COM"),
    ("DAVAO DEL NORTE", "DAV"),
    ("DAVAO DEL SUR", "DAS"),
    ("DAVAO OCCIDENTAL", "DVO"),
    ("DAVAO ORIENTAL", "DAO"),
    ("DINAGAT ISLANDS", "DIN"),
    ("EASTERN SAMAR", "EAS"),
    ("GUIMARAS", "GUI"),
    ("IFUGAO", "IFU"),
    ("ILOCOS NORTE", "ILN"),
    ("ILOCOS SUR", "ILS"),
    ("ILOILO", "ILI"),
    ("ISABELA", "ISA"),
    ("KALINGA", "KAL"),
    ("LA UNION", "LUN"),
    ("LAGUNA", "LAG"),
    ("LANAO DEL NORTE", "LAN"),
    ("LANAO DEL SUR", "LAS"),
    ("LEYTE", "LEY"),
    ("MAGUINDANAO DEL NORTE", "MGN"),
    ("MAGUINDANAO DEL SUR", "MGS"),
    ("MARINDUQUE", "MAD"),
    ("MASBATE", "MAS"),
    ("METRO MANILA", "NCR"),
    ("MINDORO OCCIDENTAL", "MDC"),
    ("MINDORO ORIENTAL", "MDR"),
    ("MISAMIS OCCIDENTAL", "MSC"),
    ("MISAMIS ORIENTAL", "MSR"),
    ("MOUNTAIN PROVINCE", "MOU"),
    ("NEGROS OCCIDENTAL", "NEC"),
    ("NEGROS ORIENTAL", "NER"),
    ("NORTHERN SAMAR", "NSA"),
    ("NUEVA ECIJA", "NUE"),
    ("NUEVA VIZCAYA", "NUV"),
    ("PALAWAN", "PLW"),
    ("PAMPANGA", "PAM"),
    ("PANGASINAN", "PAN"),
    ("QUEZON", "QUE"),
    ("QUIRINO", "QUI"),
    ("RIZAL", "RIZ"),
    ("ROMBLON", "ROM"),
    ("SAMAR", "WSA"),
    ("SARANGANI", "SAR"),
    ("SIQUIJOR", "SIG"),
    ("SORSOGON", "SOR"),
    ("SOUTH COTABATO", "SCO"),
    ("SOUTHERN LEYTE", "SLE"),
    ("SULTAN KUDARAT", "SUK"),
    ("SULU", "SLU"),
    ("SURIGAO DEL NORTE", "SUN"),
    ("SURIGAO DEL SUR", "SUR"),
    ("TARLAC", "TAR"),
    ("TAWI-TAWI", "TAW"),
    ("ZAMBALES", "ZMB"),
    ("ZAMBOANGA DEL NORTE", "ZAN"),
    ("ZAMBOANGA DEL SUR", "ZAS"),
    ("ZAMBOANGA SIBUGAY", "ZSI"),
];

#[cfg(test)]
mod tests {
    use super::PROVINCES;
    use std::collections::HashSet;

    // 82 ISO provinces + Metro Manila. Update this when a province is created or split.
    #[test]
    fn every_province_and_metro_manila_is_listed_once() {
        assert_eq!(PROVINCES.len(), 83);
        let names: HashSet<&str> = PROVINCES.iter().map(|(name, _)| *name).collect();
        assert_eq!(names.len(), PROVINCES.len(), "a province name is listed twice");
        assert!(PROVINCES.contains(&("METRO MANILA", "NCR")));
    }

    // Control numbers rely on these: exactly 3 letters A-Z, no digits, and no two provinces sharing a code
    #[test]
    fn province_codes_are_three_unique_letters() {
        for (name, code) in PROVINCES {
            assert!(code.len() == 3 && code.chars().all(|c| c.is_ascii_uppercase()), "{name}: bad code {code:?}");
        }
        let codes: HashSet<&str> = PROVINCES.iter().map(|(_, code)| *code).collect();
        assert_eq!(codes.len(), PROVINCES.len(), "two provinces share a code");
    }
}
