#![cfg(feature = "devtools")]

use incular_linux::{DevToolsLaunchMode, devtools_launch_mode_from, devtools_ui_candidates};
use std::path::PathBuf;

#[test]
fn explicit_flag_opens_the_ui() {
    assert_eq!(
        devtools_launch_mode_from(["app", "--devtools"], false),
        DevToolsLaunchMode::OpenUi
    );
    assert_eq!(
        devtools_launch_mode_from(["app", "--incular-devtools"], false),
        DevToolsLaunchMode::OpenUi
    );
}

#[test]
fn environment_preserves_agent_only_compatibility() {
    assert_eq!(
        devtools_launch_mode_from(["app"], true),
        DevToolsLaunchMode::AgentOnly
    );
    assert_eq!(
        devtools_launch_mode_from(["app"], false),
        DevToolsLaunchMode::Disabled
    );
}

#[test]
fn cargo_example_candidate_includes_profile_binary() {
    let candidates =
        devtools_ui_candidates(Some(PathBuf::from("/repo/target/debug/examples/gallery")));
    let expected = PathBuf::from(format!(
        "/repo/target/debug/incular-devtools{}",
        std::env::consts::EXE_SUFFIX
    ));
    assert!(candidates.contains(&expected), "{candidates:?}");
}
