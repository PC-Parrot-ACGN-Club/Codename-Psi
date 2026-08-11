//! Mechanical regression guard for the workspace dependency direction.
//!
//! Manifests are embedded at compile time so the check does not depend on the
//! working directory or on `cargo` being invocable from inside a test.

const GAME_CORE_MANIFEST: &str = include_str!("../../game_core/Cargo.toml");
const CLIENT_MANIFEST: &str = include_str!("../Cargo.toml");
const NET_MANIFEST: &str = include_str!("../../net/Cargo.toml");

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

/// Collect dependency names from every dependency table of a manifest.
fn dependency_names(manifest: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut in_dependencies = false;

    for line in manifest.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_dependencies = line.contains("dependencies");
            continue;
        }
        if !in_dependencies || line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some(name) = line.split(['=', '.']).next() else {
            continue;
        };
        let name = name.trim();
        if !name.is_empty() {
            names.push(name.to_string());
        }
    }
    names
}

// docs/test/game-infrastructure.md TC-051
#[test]
fn client_and_net_depend_on_game_core() {
    assert!(
        dependency_names(CLIENT_MANIFEST).contains(&"game_core".to_string()),
        "client -> game_core is a required edge"
    );
    assert!(
        dependency_names(NET_MANIFEST).contains(&"game_core".to_string()),
        "net -> game_core is a required edge"
    );
}

// docs/test/game-infrastructure.md TC-051
#[test]
fn game_core_depends_on_neither_client_nor_net() {
    let dependencies = dependency_names(GAME_CORE_MANIFEST);

    for forbidden in ["client", "net"] {
        assert!(
            !dependencies.contains(&forbidden.to_string()),
            "game_core -> {forbidden} would invert the dependency direction"
        );
    }
}

// docs/test/game-infrastructure.md TC-051
#[test]
fn game_core_stays_isolated_from_platform_runtimes() {
    let dependencies = dependency_names(GAME_CORE_MANIFEST);

    for forbidden in FORBIDDEN_IN_GAME_CORE {
        assert!(
            !dependencies.contains(&forbidden.to_string()),
            "game_core must stay free of the platform runtime crate {forbidden}"
        );
    }
}

// docs/test/game-infrastructure.md TC-051
#[test]
fn the_dependency_name_scan_reads_real_manifest_entries() {
    let dependencies = dependency_names(CLIENT_MANIFEST);

    assert!(
        dependencies.contains(&"bevy".to_string()),
        "the scan must actually see client's dependency table: {dependencies:?}"
    );
}
