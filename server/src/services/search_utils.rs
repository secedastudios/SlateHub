use regex::Regex;

/// Normalize common industry search terms to their singular form.
/// Only depluralize — don't cross-map between different words (e.g., actress stays actress, not actor).
/// The embedding synonyms handle cross-matching via vector similarity.
pub fn normalize_query(query: &str) -> String {
    let terms: &[(&str, &str)] = &[
        // Depluralize only — keep the root word intact for CONTAINS matching
        ("actresses", "actress"),
        ("actors", "actor"),
        ("cinematographers", "cinematographer"),
        ("directors", "director"),
        ("producers", "producer"),
        ("writers", "writer"),
        ("editors", "editor"),
        ("composers", "composer"),
        ("gaffers", "gaffer"),
        ("grips", "grip"),
        ("colorists", "colorist"),
        ("animators", "animator"),
        ("stunt performers", "stunt performer"),
        ("choreographers", "choreographer"),
        ("screenwriters", "screenwriter"),
        ("showrunners", "showrunner"),
        ("production designers", "production designer"),
        ("costume designers", "costume designer"),
        ("sound designers", "sound designer"),
        ("makeup artists", "makeup artist"),
        ("filmmakers", "filmmaker"),
        ("photographers", "photographer"),
        ("videographers", "videographer"),
        ("models", "model"),
        // Common abbreviations — depluralize only
        ("dps", "dp"),
        ("dops", "dop"),
        ("ads", "ad"),
        ("pas", "pa"),
    ];
    let mut result = query.to_lowercase();
    for (plural, singular) in terms {
        let re = Regex::new(&format!(r"(?i)\b{}\b", regex::escape(plural))).unwrap();
        result = re.replace_all(&result, *singular).to_string();
    }
    result
}

/// Parsed structured filters from a natural language search query.
#[derive(Debug, Default, Clone)]
pub struct ParsedQuery {
    pub location: Option<String>,
    pub gender: Option<String>,
    pub age_min: Option<i32>,
    pub age_max: Option<i32>,
    pub hair_color: Option<String>,
    pub eye_color: Option<String>,
    pub body_type: Option<String>,
    pub cleaned: String,
}

/// Parse natural language query into structured filters + cleaned search text.
/// Handles: "blonde female actors ages 20-30 in Berlin", "bald men with blue eyes in LA"
pub fn parse_query(query: &str) -> ParsedQuery {
    let mut cleaned = query.to_string();
    let mut parsed = ParsedQuery::default();

    // Location: "in <city/region>" at end of query (must be parsed first before other removals)
    let loc_re = Regex::new(r"(?i)\bin\s+(.+)$").unwrap();
    if let Some(caps) = loc_re.captures(&cleaned) {
        parsed.location = caps.get(1).map(|m| m.as_str().trim().to_string());
        cleaned = loc_re.replace(&cleaned, "").to_string();
    }

    // Age range: "age(s) 20-30", "ages 20 to 30", "age range 25-35"
    let age_re = Regex::new(r"(?i)\bage(?:s|\s+range)?\s+(\d+)\s*[-\u{2013}to]+\s*(\d+)").unwrap();
    if let Some(caps) = age_re.captures(&cleaned) {
        parsed.age_min = caps.get(1).and_then(|m| m.as_str().parse().ok());
        parsed.age_max = caps.get(2).and_then(|m| m.as_str().parse().ok());
        cleaned = age_re.replace(&cleaned, "").to_string();
    }

    // Gender: "male", "female", "non-binary", "men", "women", "man", "woman"
    let gender_re =
        Regex::new(r"(?i)\b(male|female|non[- ]?binary|men|women|man|woman)\b").unwrap();
    if let Some(m) = gender_re.find(&cleaned) {
        let g = m.as_str().to_lowercase();
        parsed.gender = Some(match g.as_str() {
            "male" | "man" | "men" => "Male".to_string(),
            "female" | "woman" | "women" => "Female".to_string(),
            _ => "Non-Binary".to_string(),
        });
        cleaned = gender_re.replace(&cleaned, "").to_string();
    }

    // Hair color: "blonde hair", "brown-haired", "with red hair", "bald"
    let hair_re = Regex::new(
        r"(?i)\b(black|brown|blonde|blond|red|gray|grey|white|bald)(?:[- ]?haired|\s+hair)?\b",
    )
    .unwrap();
    if let Some(m) = hair_re.find(&cleaned) {
        let h = m.as_str().to_lowercase();
        parsed.hair_color = Some(
            match h.as_str() {
                s if s.contains("black") => "Black",
                s if s.contains("brown") => "Brown",
                s if s.contains("blond") => "Blonde",
                s if s.contains("red") => "Red",
                s if s.contains("gray") || s.contains("grey") => "Gray",
                s if s.contains("white") => "White",
                s if s.contains("bald") => "Bald",
                _ => "Other",
            }
            .to_string(),
        );
        cleaned = hair_re.replace(&cleaned, "").to_string();
    }

    // Eye color: "blue eyes", "brown-eyed", "with green eyes"
    let eye_re = Regex::new(
        r"(?i)\b(?:with\s+)?(brown|blue|green|hazel|gray|grey|black)(?:[- ]?eyed|\s+eyes?)\b",
    )
    .unwrap();
    if let Some(caps) = eye_re.captures(&cleaned) {
        let e = caps.get(1).unwrap().as_str().to_lowercase();
        parsed.eye_color = Some(
            match e.as_str() {
                "brown" => "Brown",
                "blue" => "Blue",
                "green" => "Green",
                "hazel" => "Hazel",
                "gray" | "grey" => "Gray",
                "black" => "Black",
                _ => "Other",
            }
            .to_string(),
        );
        cleaned = eye_re.replace(&cleaned, "").to_string();
    }

    // Body type: "athletic", "slim", "muscular", "petite", "plus size", "curvy"
    let body_re = Regex::new(
        r"(?i)\b(athletic|average|slim|slender|curvy|muscular|petite|plus[- ]?size|tall)\b",
    )
    .unwrap();
    if let Some(m) = body_re.find(&cleaned) {
        let b = m.as_str().to_lowercase();
        parsed.body_type = Some(
            match b.as_str() {
                "athletic" => "Athletic",
                "average" => "Average",
                "slim" => "Slim",
                "slender" => "Slender",
                "curvy" => "Curvy",
                "muscular" => "Muscular",
                "petite" => "Petite",
                s if s.contains("plus") => "Plus Size",
                "tall" => "Tall",
                _ => "Other",
            }
            .to_string(),
        );
        cleaned = body_re.replace(&cleaned, "").to_string();
    }

    // Clean up filler words left behind
    let filler_re = Regex::new(r"(?i)\b(with|and|who|are|is|that|the|a|an)\b").unwrap();
    cleaned = filler_re.replace_all(&cleaned, "").to_string();

    // Normalize role plurals
    cleaned = normalize_query(&cleaned);

    // Collapse whitespace
    cleaned = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    parsed.cleaned = cleaned;
    parsed
}

/// Simple location-only extraction for non-people searches.
pub fn extract_location(query: &str) -> (Option<String>, String) {
    let loc_re = Regex::new(r"(?i)\bin\s+(.+)$").unwrap();
    if let Some(caps) = loc_re.captures(query) {
        let location = caps.get(1).map(|m| m.as_str().trim().to_string());
        let cleaned = loc_re.replace(query, "").to_string();
        let cleaned = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
        (location, cleaned)
    } else {
        (None, query.to_string())
    }
}
