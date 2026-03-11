//! Video platform URL parsing and embed generation.
//!
//! Extracts video IDs from popular platforms and generates
//! thumbnail URLs and embed URLs for display.

use regex::Regex;
use reqwest::Client;
use serde::Deserialize;
use std::sync::LazyLock;
use tracing::debug;

static HTTP_CLIENT: LazyLock<Client> = LazyLock::new(|| {
    Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .expect("failed to build HTTP client")
});

/// Parsed video info from a URL
pub struct VideoInfo {
    pub platform: &'static str,
    pub video_id: String,
}

// YouTube patterns: youtube.com/watch?v=ID, youtu.be/ID, youtube.com/embed/ID, youtube.com/shorts/ID
static YT_LONG: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:youtube\.com/(?:watch\?.*v=|embed/|shorts/))([a-zA-Z0-9_-]{11})").unwrap());
static YT_SHORT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"youtu\.be/([a-zA-Z0-9_-]{11})").unwrap());

// Vimeo patterns: vimeo.com/ID, player.vimeo.com/video/ID
static VIMEO: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"vimeo\.com/(?:video/)?(\d+)").unwrap());

// TikTok patterns: tiktok.com/@user/video/ID
static TIKTOK: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"tiktok\.com/@[^/]+/video/(\d+)").unwrap());

// Dailymotion patterns: dailymotion.com/video/ID
static DAILYMOTION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"dailymotion\.com/video/([a-zA-Z0-9]+)").unwrap());

/// Parse a video URL and extract platform + video ID.
/// Returns `None` if the URL is not from a supported platform.
pub fn parse_video_url(url: &str) -> Option<VideoInfo> {
    // YouTube
    if let Some(caps) = YT_LONG.captures(url) {
        return Some(VideoInfo {
            platform: "youtube",
            video_id: caps[1].to_string(),
        });
    }
    if let Some(caps) = YT_SHORT.captures(url) {
        return Some(VideoInfo {
            platform: "youtube",
            video_id: caps[1].to_string(),
        });
    }

    // Vimeo
    if let Some(caps) = VIMEO.captures(url) {
        return Some(VideoInfo {
            platform: "vimeo",
            video_id: caps[1].to_string(),
        });
    }

    // TikTok
    if let Some(caps) = TIKTOK.captures(url) {
        return Some(VideoInfo {
            platform: "tiktok",
            video_id: caps[1].to_string(),
        });
    }

    // Dailymotion
    if let Some(caps) = DAILYMOTION.captures(url) {
        return Some(VideoInfo {
            platform: "dailymotion",
            video_id: caps[1].to_string(),
        });
    }

    None
}

/// Generate a thumbnail URL for a given platform and video ID.
pub fn thumbnail_url(platform: &str, video_id: &str) -> String {
    match platform {
        "youtube" => format!("https://img.youtube.com/vi/{}/mqdefault.jpg", video_id),
        "vimeo" => format!("https://vumbnail.com/{}.jpg", video_id),
        "dailymotion" => format!("https://www.dailymotion.com/thumbnail/video/{}", video_id),
        _ => String::new(),
    }
}

/// Generate an embed URL for a given platform and video ID.
pub fn embed_url(platform: &str, video_id: &str) -> String {
    match platform {
        "youtube" => format!("https://www.youtube.com/embed/{}?autoplay=1", video_id),
        "vimeo" => format!("https://player.vimeo.com/video/{}?autoplay=1", video_id),
        "tiktok" => format!("https://www.tiktok.com/embed/v2/{}", video_id),
        "dailymotion" => format!("https://www.dailymotion.com/embed/video/{}?autoplay=1", video_id),
        _ => String::new(),
    }
}

/// oEmbed response — we only need the title field.
#[derive(Deserialize)]
struct OEmbedResponse {
    title: Option<String>,
}

/// Fetch the video title from the platform's oEmbed API.
/// Returns `None` on any failure (network, parse, unsupported platform).
pub async fn fetch_video_title(platform: &str, url: &str) -> Option<String> {
    let oembed_url = match platform {
        "youtube" => format!(
            "https://www.youtube.com/oembed?url={}&format=json",
            urlencoding::encode(url)
        ),
        "vimeo" => format!(
            "https://vimeo.com/api/oembed.json?url={}",
            urlencoding::encode(url)
        ),
        "tiktok" => format!(
            "https://www.tiktok.com/oembed?url={}",
            urlencoding::encode(url)
        ),
        "dailymotion" => format!(
            "https://www.dailymotion.com/services/oembed?url={}&format=json",
            urlencoding::encode(url)
        ),
        _ => return None,
    };

    match HTTP_CLIENT.get(&oembed_url).send().await {
        Ok(resp) if resp.status().is_success() => {
            let data: OEmbedResponse = resp.json().await.ok()?;
            let title = data.title?.trim().to_string();
            if title.is_empty() {
                None
            } else {
                debug!("Fetched video title: {}", title);
                Some(title)
            }
        }
        Ok(resp) => {
            debug!("oEmbed request returned status {}", resp.status());
            None
        }
        Err(e) => {
            debug!("oEmbed request failed: {}", e);
            None
        }
    }
}

/// Human-readable platform name.
pub fn platform_name(platform: &str) -> &'static str {
    match platform {
        "youtube" => "YouTube",
        "vimeo" => "Vimeo",
        "tiktok" => "TikTok",
        "dailymotion" => "Dailymotion",
        _ => "Video",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_youtube_long() {
        let info = parse_video_url("https://www.youtube.com/watch?v=dQw4w9WgXcQ").unwrap();
        assert_eq!(info.platform, "youtube");
        assert_eq!(info.video_id, "dQw4w9WgXcQ");
    }

    #[test]
    fn test_youtube_short() {
        let info = parse_video_url("https://youtu.be/dQw4w9WgXcQ").unwrap();
        assert_eq!(info.platform, "youtube");
        assert_eq!(info.video_id, "dQw4w9WgXcQ");
    }

    #[test]
    fn test_youtube_shorts() {
        let info = parse_video_url("https://youtube.com/shorts/dQw4w9WgXcQ").unwrap();
        assert_eq!(info.platform, "youtube");
        assert_eq!(info.video_id, "dQw4w9WgXcQ");
    }

    #[test]
    fn test_vimeo() {
        let info = parse_video_url("https://vimeo.com/123456789").unwrap();
        assert_eq!(info.platform, "vimeo");
        assert_eq!(info.video_id, "123456789");
    }

    #[test]
    fn test_tiktok() {
        let info = parse_video_url("https://www.tiktok.com/@user/video/7123456789012345678").unwrap();
        assert_eq!(info.platform, "tiktok");
        assert_eq!(info.video_id, "7123456789012345678");
    }

    #[test]
    fn test_unsupported() {
        assert!(parse_video_url("https://example.com/video").is_none());
    }
}
