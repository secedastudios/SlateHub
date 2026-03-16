/// Social media platform registry for profile links.
///
/// Each platform has an ID (stored in DB), display name, optional base URL
/// for handle→URL expansion, placeholder text, and an inline SVG icon.

pub struct SocialPlatform {
    pub id: &'static str,
    pub name: &'static str,
    /// URL template with `{}` as handle placeholder. `None` = full URL required.
    pub base_url: Option<&'static str>,
    pub placeholder: &'static str,
    pub icon_svg: &'static str,
}

pub const SOCIAL_PLATFORMS: &[SocialPlatform] = &[
    // ── General Social ──
    SocialPlatform {
        id: "youtube",
        name: "YouTube",
        base_url: Some("https://youtube.com/@{}"),
        placeholder: "@channel or full URL",
        icon_svg: r#"<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M22.54 6.42a2.78 2.78 0 0 0-1.94-2C18.88 4 12 4 12 4s-6.88 0-8.6.46a2.78 2.78 0 0 0-1.94 2A29 29 0 0 0 1 11.75a29 29 0 0 0 .46 5.33A2.78 2.78 0 0 0 3.4 19.13C5.12 19.56 12 19.56 12 19.56s6.88 0 8.6-.46a2.78 2.78 0 0 0 1.94-2 29 29 0 0 0 .46-5.25 29 29 0 0 0-.46-5.43z"/><polygon points="9.75 15.02 15.5 11.75 9.75 8.48 9.75 15.02" fill="currentColor" stroke="none"/></svg>"#,
    },
    SocialPlatform {
        id: "tiktok",
        name: "TikTok",
        base_url: Some("https://tiktok.com/@{}"),
        placeholder: "@username or full URL",
        icon_svg: r#"<svg width="20" height="20" viewBox="0 0 24 24" fill="currentColor"><path d="M19.59 6.69a4.83 4.83 0 0 1-3.77-4.25V2h-3.45v13.67a2.89 2.89 0 0 1-2.88 2.5 2.89 2.89 0 0 1-2.89-2.89 2.89 2.89 0 0 1 2.89-2.89c.28 0 .54.04.79.1V9.01a6.27 6.27 0 0 0-.79-.05 6.34 6.34 0 0 0-6.34 6.34 6.34 6.34 0 0 0 6.34 6.34 6.34 6.34 0 0 0 6.34-6.34V8.75a8.18 8.18 0 0 0 4.76 1.52V6.84a4.84 4.84 0 0 1-1-.15z"/></svg>"#,
    },
    SocialPlatform {
        id: "instagram",
        name: "Instagram",
        base_url: Some("https://instagram.com/{}"),
        placeholder: "username or full URL",
        icon_svg: r#"<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><rect x="2" y="2" width="20" height="20" rx="5" ry="5"/><circle cx="12" cy="12" r="5"/><circle cx="17.5" cy="6.5" r="1.5" fill="currentColor" stroke="none"/></svg>"#,
    },
    SocialPlatform {
        id: "vimeo",
        name: "Vimeo",
        base_url: Some("https://vimeo.com/{}"),
        placeholder: "username or full URL",
        icon_svg: r#"<svg width="20" height="20" viewBox="0 0 24 24" fill="currentColor"><path d="M23.977 6.416c-.105 2.338-1.739 5.543-4.894 9.609C15.9 20.058 12.985 22 10.616 22c-1.46 0-2.697-1.35-3.71-4.046-.675-2.478-1.35-4.957-2.025-7.435C4.23 7.87 3.52 6.545 2.76 6.545c-.19 0-.855.4-1.992 1.198L0 6.773c1.252-1.1 2.488-2.2 3.707-3.3 1.67-1.444 2.924-2.204 3.76-2.28 1.974-.19 3.19 1.161 3.645 4.055.492 3.124.833 5.069 1.022 5.834.568 2.58 1.19 3.87 1.87 3.87.527 0 1.32-.834 2.377-2.504 1.053-1.67 1.617-2.942 1.69-3.816.15-1.444-.416-2.168-1.69-2.168-.601 0-1.22.138-1.856.412 1.232-4.036 3.587-5.997 7.067-5.882 2.58.076 3.796 1.749 3.645 5.022z"/></svg>"#,
    },
    SocialPlatform {
        id: "linkedin",
        name: "LinkedIn",
        base_url: Some("https://linkedin.com/in/{}"),
        placeholder: "username or full URL",
        icon_svg: r#"<svg width="20" height="20" viewBox="0 0 24 24" fill="currentColor"><path d="M20.447 20.452h-3.554v-5.569c0-1.328-.027-3.037-1.852-3.037-1.853 0-2.136 1.445-2.136 2.939v5.667H9.351V9h3.414v1.561h.046c.477-.9 1.637-1.85 3.37-1.85 3.601 0 4.267 2.37 4.267 5.455v6.286zM5.337 7.433a2.062 2.062 0 0 1-2.063-2.065 2.064 2.064 0 1 1 2.063 2.065zm1.782 13.019H3.555V9h3.564v11.452zM22.225 0H1.771C.792 0 0 .774 0 1.729v20.542C0 23.227.792 24 1.771 24h20.451C23.2 24 24 23.227 24 22.271V1.729C24 .774 23.2 0 22.222 0h.003z"/></svg>"#,
    },
    SocialPlatform {
        id: "x",
        name: "X",
        base_url: Some("https://x.com/{}"),
        placeholder: "username or full URL",
        icon_svg: r#"<svg width="20" height="20" viewBox="0 0 24 24" fill="currentColor"><path d="M18.244 2.25h3.308l-7.227 8.26 8.502 11.24H16.17l-5.214-6.817L4.99 21.75H1.68l7.73-8.835L1.254 2.25H8.08l4.713 6.231zm-1.161 17.52h1.833L7.084 4.126H5.117z"/></svg>"#,
    },
    SocialPlatform {
        id: "bluesky",
        name: "Bluesky",
        base_url: Some("https://bsky.app/profile/{}"),
        placeholder: "handle.bsky.social or full URL",
        icon_svg: r#"<svg width="20" height="20" viewBox="0 0 24 24" fill="currentColor"><path d="M12 10.8c-1.087-2.114-4.046-6.053-6.798-7.995C2.566.944 1.561 1.266.902 1.565.139 1.908 0 3.08 0 3.768c0 .69.378 5.65.624 6.479.785 2.627 3.6 3.476 6.158 3.129-4.397.62-8.258 2.129-4.476 7.53C5.705 24.89 10.254 18.87 12 15.47c1.746 3.4 6.037 9.257 9.694 5.436 3.782-5.4-.08-6.91-4.476-7.53 2.558.347 5.373-.502 6.159-3.129.245-.828.623-5.789.623-6.479 0-.688-.139-1.86-.902-2.203-.659-.3-1.664-.621-4.3 1.24C16.046 4.748 13.087 8.687 12 10.8z"/></svg>"#,
    },

    // ── Industry ──
    SocialPlatform {
        id: "imdb",
        name: "IMDb",
        base_url: None,
        placeholder: "https://imdb.com/name/nm...",
        icon_svg: r#"<svg width="20" height="20" viewBox="0 0 24 24" fill="currentColor"><path d="M14.31 9.588v4.823H15.62V9.588zm-6.462 0h-.003v4.826h1.107V12.2l.458 2.214h.936l.42-2.282v2.282h1.092V9.588h-1.59l-.347 2.04-.347-2.04zm-2.073 0H4.45v4.823h1.325c.39 0 .679-.078.867-.233.19-.157.283-.425.283-.804V10.62c0-.38-.094-.645-.283-.8-.188-.157-.477-.233-.867-.233zM1.5 7.5h21v9h-21z"/><path d="M0 4.5v15h24v-15zm22.5 13.5h-21v-9h21z" fill-rule="evenodd"/><path d="M19.07 9.588h-1.595v4.823h1.595c.468 0 .81-.093 1.03-.278.218-.186.327-.47.327-.853V10.72c0-.382-.11-.667-.327-.853-.22-.185-.562-.278-1.03-.278z"/></svg>"#,
    },
    SocialPlatform {
        id: "letterboxd",
        name: "Letterboxd",
        base_url: Some("https://letterboxd.com/{}"),
        placeholder: "username or full URL",
        icon_svg: r#"<svg width="20" height="20" viewBox="0 0 24 24" fill="currentColor"><path d="M8.224 14.352a4.985 4.985 0 0 1 0-4.704l.068-.123A4.978 4.978 0 0 0 4 12a4.978 4.978 0 0 0 4.292 2.475zm7.552 0A4.978 4.978 0 0 0 20 12a4.978 4.978 0 0 0-4.292-2.475l.068.123a4.985 4.985 0 0 1 0 4.704zM12 16a4.99 4.99 0 0 0 4.292-2.452 4.985 4.985 0 0 0 0-3.096A4.99 4.99 0 0 0 12 8a4.99 4.99 0 0 0-4.292 2.452 4.985 4.985 0 0 0 0 3.096A4.99 4.99 0 0 0 12 16z"/></svg>"#,
    },
    SocialPlatform {
        id: "tmdb",
        name: "TMDb",
        base_url: None,
        placeholder: "https://themoviedb.org/person/...",
        icon_svg: r#"<svg width="20" height="20" viewBox="0 0 24 24" fill="currentColor"><path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm-1 14H9V8h2v8zm5 0h-2V8h2v8z"/></svg>"#,
    },
    SocialPlatform {
        id: "backstage",
        name: "Backstage",
        base_url: None,
        placeholder: "https://backstage.com/u/...",
        icon_svg: r#"<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><path d="M8 12l3 3 5-5"/></svg>"#,
    },
    SocialPlatform {
        id: "mandy",
        name: "Mandy",
        base_url: None,
        placeholder: "https://mandy.com/...",
        icon_svg: r#"<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><rect x="2" y="3" width="20" height="18" rx="2"/><path d="M8 7v10M12 7v10M16 7v10"/></svg>"#,
    },
    SocialPlatform {
        id: "crewunited",
        name: "Crew United",
        base_url: None,
        placeholder: "https://crewunited.com/...",
        icon_svg: r#"<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M17 21v-2a4 4 0 0 0-4-4H5a4 4 0 0 0-4 4v2"/><circle cx="9" cy="7" r="4"/><path d="M23 21v-2a4 4 0 0 0-3-3.87"/><path d="M16 3.13a4 4 0 0 1 0 7.75"/></svg>"#,
    },

    // ── Fallback ──
    SocialPlatform {
        id: "other",
        name: "Other",
        base_url: None,
        placeholder: "https://...",
        icon_svg: r#"<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71"/><path d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71"/></svg>"#,
    },
];

/// Look up a platform by its ID. Returns the "other" platform as fallback.
pub fn find_platform(id: &str) -> &'static SocialPlatform {
    SOCIAL_PLATFORMS
        .iter()
        .find(|p| p.id == id)
        .unwrap_or(SOCIAL_PLATFORMS.last().unwrap())
}

/// Expand a handle to a full URL using the platform's base_url template.
/// If the value already looks like a URL, returns it as-is.
pub fn expand_url(platform_id: &str, value: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        return String::new();
    }

    // Already a URL — only allow http/https schemes
    if value.starts_with("http://") || value.starts_with("https://") {
        return value.to_string();
    }

    // Reject dangerous URI schemes
    let lower = value.to_lowercase();
    if lower.starts_with("javascript:") || lower.starts_with("data:") || lower.starts_with("vbscript:") {
        return String::new();
    }

    let platform = find_platform(platform_id);
    match platform.base_url {
        Some(template) => {
            // Strip leading @ if present
            let handle = value.trim_start_matches('@');
            template.replace("{}", handle)
        }
        None => value.to_string(),
    }
}
