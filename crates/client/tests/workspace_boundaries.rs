//! Mechanical regression guard for the workspace dependency direction.
//!
//! The graph comes from `cargo metadata`, not from reading the manifests as
//! text. Cargo reports the real package name of every dependency, so a rename
//! (`quiet = { package = "bevy" }`) cannot slip a forbidden crate past these
//! assertions, and target-specific or workspace-inherited tables are already
//! resolved by the time we see them.

use std::process::Command;

use serde_json::Value;

/// Platform runtime crates that must never reach the pure rules crate.
const FORBIDDEN_IN_GAME_CORE: [&str; 8] = [
    "bevy",
    "client",
    "net",
    "winit",
    "wgpu",
    "directories",
    "ggrs",
    "local-ip-address",
];

/// The workspace's own packages and their declared dependencies.
///
/// `--no-deps` keeps this to the workspace members, so the check neither
/// resolves nor touches the registry. `--locked` keeps it honest about the
/// committed lockfile, matching how CI builds.
fn workspace_metadata() -> Value {
    let output = Command::new(env!("CARGO"))
        .args(["metadata", "--no-deps", "--format-version", "1", "--locked"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("cargo metadata is runnable from the test");

    assert!(
        output.status.success(),
        "cargo metadata failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("cargo metadata emits JSON")
}

/// Real package names a crate depends on at build time.
///
/// Dev- and build-dependencies are excluded: a required edge must not be
/// satisfiable by a test-only dependency, and a forbidden crate is only
/// forbidden where it would ship.
fn normal_dependencies(metadata: &Value, package: &str) -> Vec<String> {
    let packages = metadata["packages"]
        .as_array()
        .expect("metadata lists packages");
    let entry = packages
        .iter()
        .find(|candidate| candidate["name"] == package)
        .unwrap_or_else(|| panic!("{package} is a workspace member"));

    entry["dependencies"]
        .as_array()
        .expect("a package lists dependencies")
        .iter()
        .filter(|dependency| dependency["kind"].is_null())
        .map(|dependency| {
            dependency["name"]
                .as_str()
                .expect("a dependency has a name")
                .to_string()
        })
        .collect()
}

// integration-system/build-and-startup::TC-002
#[test]
fn client_and_net_depend_on_game_core() {
    let metadata = workspace_metadata();

    for dependent in ["client", "net"] {
        assert!(
            normal_dependencies(&metadata, dependent).contains(&"game_core".to_string()),
            "{dependent} -> game_core is a required edge"
        );
    }
}

// integration-system/build-and-startup::TC-002
#[test]
fn game_core_depends_on_neither_client_nor_net() {
    let dependencies = normal_dependencies(&workspace_metadata(), "game_core");

    for forbidden in ["client", "net"] {
        assert!(
            !dependencies.contains(&forbidden.to_string()),
            "game_core -> {forbidden} would invert the dependency direction"
        );
    }
}

// integration-system/build-and-startup::TC-002
#[test]
fn game_core_stays_isolated_from_platform_runtimes() {
    let dependencies = normal_dependencies(&workspace_metadata(), "game_core");

    for forbidden in FORBIDDEN_IN_GAME_CORE {
        assert!(
            !dependencies.contains(&forbidden.to_string()),
            "game_core must stay free of the platform runtime crate {forbidden}"
        );
    }
}

// integration-system/build-and-startup::TC-002
#[test]
fn the_dependency_scan_sees_real_package_names_and_skips_dev_only_edges() {
    let metadata = workspace_metadata();
    let client = normal_dependencies(&metadata, "client");

    assert!(
        client.contains(&"bevy".to_string()),
        "the scan must actually see client's dependencies: {client:?}"
    );
    // `tempfile` is a dev-dependency of client only. If it showed up here, the
    // required-edge assertions would also pass for a `game_core` moved into
    // `[dev-dependencies]`, which is no longer a runtime edge.
    assert!(
        !client.contains(&"tempfile".to_string()),
        "a dev-only dependency must never satisfy a required edge: {client:?}"
    );

    // `net` is declared as an optional dependency behind a feature. It is still
    // a normal edge, which is what makes the forbidden-crate checks meaningful:
    // an optional dependency ships as soon as its feature is on.
    assert!(
        client.contains(&"net".to_string()),
        "an optional dependency is still a normal edge: {client:?}"
    );
}
