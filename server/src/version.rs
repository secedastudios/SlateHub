pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const BUILD_TIMESTAMP: &str = env!("BUILD_TIMESTAMP");
pub const GIT_HASH: &str = env!("GIT_HASH");
pub const GIT_BRANCH: &str = env!("GIT_BRANCH");
pub const BUILD_NUMBER: &str = env!("BUILD_NUMBER");

pub fn full_version() -> String {
    format!("{}.{}", VERSION, BUILD_NUMBER)
}

pub fn build_info() -> String {
    format!("v{} build {} ({} @ {} built {})", VERSION, BUILD_NUMBER, GIT_HASH, GIT_BRANCH, BUILD_TIMESTAMP)
}
