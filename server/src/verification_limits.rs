/// Upload limits based on a person's verification status.
pub struct UploadLimits {
    pub max_photos: Option<usize>,  // None = unlimited
    pub max_reels: Option<usize>,   // None = unlimited
}

/// Returns the upload limits for a given verification status.
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
