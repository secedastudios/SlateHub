use regex::Regex;

/// Normalize common industry search terms to their singular profile form.
/// "directors" → "director", "actors" → "actor", "actresses" → "actor"
pub fn normalize_query(query: &str) -> String {
    let terms: &[(&str, &str)] = &[
        ("actresses", "actor"), ("actress", "actor"), ("actors", "actor"),
        ("cinematographers", "cinematographer"),
        ("directors", "director"), ("producers", "producer"),
        ("writers", "writer"), ("editors", "editor"),
        ("composers", "composer"), ("gaffers", "gaffer"),
        ("grips", "grip"), ("colorists", "colorist"),
        ("animators", "animator"), ("stunt performers", "stunt performer"),
        ("choreographers", "choreographer"), ("screenwriters", "screenwriter"),
        ("showrunners", "showrunner"),
        ("production designers", "production designer"),
        ("costume designers", "costume designer"),
        ("sound designers", "sound designer"),
        ("makeup artists", "makeup artist"),
        ("filmmakers", "filmmaker"), ("photographers", "photographer"),
        ("videographers", "videographer"), ("models", "model"),
        // Common abbreviations
        ("dps", "dp"), ("dops", "dop"),
        ("ads", "ad"), ("pas", "pa"),
    ];
    let mut result = query.to_lowercase();
    for (plural, singular) in terms {
        let re = Regex::new(&format!(r"(?i)\b{}\b", regex::escape(plural))).unwrap();
        result = re.replace_all(&result, *singular).to_string();
    }
    result
}
