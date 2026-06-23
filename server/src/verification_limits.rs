//! Media upload quotas tied to a person's verification status.
//!
//! Maps the `person.verification_status` string to photo/reel caps:
//! identity-verified members get expanded limits, everyone else the free
//! tier. `routes::profile` and `routes::media` consult these limits before
//! accepting uploads and pass them to templates so the UI can show remaining
//! slots.

/// Upload limits based on a person's verification status.
pub struct UploadLimits {
    /// Maximum number of profile photos; `None` means unlimited.
    pub max_photos: Option<usize>,
    /// Maximum number of video reels; `None` means unlimited.
    pub max_reels: Option<usize>,
}

/// Returns the upload limits for a given verification status: `"identity"`
/// grants 20 photos and unlimited reels; any other status gets 3 of each.
pub fn limits_for_status(verification_status: &str) -> UploadLimits {
    match verification_status {
        "identity" => UploadLimits {
            max_photos: Some(20),
            max_reels: None,
        },
        _ => UploadLimits {
            max_photos: Some(3),
            max_reels: Some(3),
        },
    }
}

/// Whether the given status counts as identity-verified.
pub fn is_identity_verified(verification_status: &str) -> bool {
    verification_status == "identity"
}
