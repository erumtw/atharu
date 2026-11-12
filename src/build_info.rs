//! What this binary was built from (baked in by build.rs). One source for `atharu version` and Settings › About,
//! kept outside the slices so the CLI and the settings feature don't import each other.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
/// Release tag (`v0.1.0`); `None` = a dev build (D40).
pub const TAG: Option<&str> = option_env!("ATHARU_GIT_TAG");
pub const SHA: &str = or_unknown(option_env!("ATHARU_GIT_SHA"));
pub const BUILT: &str = or_unknown(option_env!("ATHARU_BUILT"));

const fn or_unknown(v: Option<&'static str>) -> &'static str {
    match v {
        Some(s) => s,
        None => "unknown",
    }
}
