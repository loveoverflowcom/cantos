//! Centralized repository facts that commands share. Each value mirrors a file in the
//! repository (a manifest, `Trunk.toml`, a script or a README); tests compare them.

use std::net::{Ipv4Addr, SocketAddr};

/// Rust version every package declares and CI pins (`rust-version` / `RUSTUP_TOOLCHAIN`).
pub const REQUIRED_RUST: &str = "1.87";
/// Trunk version the web READMEs document.
pub const TRUNK_VERSION: &str = "0.21.14";
/// Reviewed installed Gradle version for the package-local CMP preview (no wrapper download).
pub const GRADLE_VERSION: &str = "8.14.3";
pub const UI_LEPTOS_DIR: &str = "packages/ui-system/leptos";
pub const UI_CMP_DIR: &str = "packages/ui-system/cmp";
pub const UI_PREVIEW_PORT: u16 = 8083;
pub const UI_PREVIEW_MARKER: &str = "<title>Cantos UI foundation preview</title>";
pub const UI_PREVIEW_DIST: &str = "../../../target/ui-foundation-preview";
pub const WASM_TARGET: &str = "wasm32-unknown-unknown";
/// Default comparison base for `preflight --changed-only`.
pub const DEFAULT_BASE: &str = "origin/develop";

pub const API_PACKAGE: &str = "cantos-api";
pub const UI_PACKAGE: &str = "cantos-ui";
pub const API_SERVER_BIN: &str = "cantos-server";
pub const API_PORT: u16 = 8080;
/// Written by `scripts/test_postgres.py --keep`; read by `scripts/serve_studio.py`.
pub const TEST_CLUSTER_RECORD: &str = "target/revision-evidence/run.json";

pub fn loopback(port: u16) -> SocketAddr {
    SocketAddr::from((Ipv4Addr::LOCALHOST, port))
}

/// A Cantos product with a Web client and a reserved mobile client.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum App {
    Studio,
    Theatre,
}

impl App {
    pub const ALL: [App; 2] = [App::Studio, App::Theatre];

    /// Name used by the command line (`cargo xtask build cantos-studio --web`).
    pub fn cli_name(self) -> &'static str {
        match self {
            App::Studio => "cantos-studio",
            App::Theatre => "cantos-theatre",
        }
    }

    pub fn web_package(self) -> &'static str {
        match self {
            App::Studio => "cantos-studio",
            App::Theatre => "cantos-theatre",
        }
    }

    pub fn web_dir(self) -> &'static str {
        match self {
            App::Studio => "apps/cantos-studio/web",
            App::Theatre => "apps/cantos-theatre/web",
        }
    }

    pub fn mobile_dir(self) -> &'static str {
        match self {
            App::Studio => "apps/cantos-studio/mobile",
            App::Theatre => "apps/cantos-theatre/mobile",
        }
    }

    /// `trunk serve` port; must equal `[serve] port` in the app's `Trunk.toml`.
    pub fn web_port(self) -> u16 {
        match self {
            App::Studio => 8081,
            App::Theatre => 8082,
        }
    }

    /// Text that identifies this app's own `index.html` during readiness probing.
    pub fn web_marker(self) -> &'static str {
        match self {
            App::Studio => "<title>Cantos Studio</title>",
            App::Theatre => "<title>Cantos</title>",
        }
    }

    pub fn web_origin(self) -> String {
        format!("http://{}", loopback(self.web_port()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::Workspace;

    fn read(relative: &str) -> String {
        let workspace = Workspace::discover().unwrap();
        std::fs::read_to_string(workspace.path(relative)).unwrap()
    }

    #[test]
    fn web_ports_and_markers_match_the_app_files() {
        for app in App::ALL {
            let trunk = read(&format!("{}/Trunk.toml", app.web_dir()));
            assert!(
                trunk.contains(&format!("port = {}", app.web_port())),
                "{} Trunk.toml does not declare port {}",
                app.cli_name(),
                app.web_port()
            );
            let index = read(&format!("{}/index.html", app.web_dir()));
            assert!(index.contains(app.web_marker()));
            let manifest = read(&format!("{}/Cargo.toml", app.web_dir()));
            assert!(manifest.contains(&format!("name = \"{}\"", app.web_package())));
        }
    }

    #[test]
    fn leptos_preview_marker_matches_its_package_local_entrypoint() {
        assert!(read(&format!("{UI_LEPTOS_DIR}/preview.html")).contains(UI_PREVIEW_MARKER));
        assert_eq!(UI_PREVIEW_PORT, 8083);
        assert!(!App::ALL.iter().any(|app| app.web_port() == UI_PREVIEW_PORT));
    }

    #[test]
    fn studio_proxy_targets_the_api_port() {
        let trunk = read("apps/cantos-studio/web/Trunk.toml");
        assert!(trunk.contains(&format!("http://127.0.0.1:{API_PORT}/api/")));
    }

    #[test]
    fn rust_version_matches_every_package() {
        for manifest in [
            "services/cantos-api/Cargo.toml",
            "apps/cantos-studio/web/Cargo.toml",
            "apps/cantos-theatre/web/Cargo.toml",
            "packages/contracts/api/Cargo.toml",
            "packages/ui-system/leptos/Cargo.toml",
            "xtask/Cargo.toml",
        ] {
            assert!(
                read(manifest).contains(&format!("rust-version = \"{REQUIRED_RUST}\"")),
                "{manifest}"
            );
        }
    }

    #[test]
    fn mobile_directories_are_still_guides_without_a_gradle_project() {
        let workspace = Workspace::discover().unwrap();
        for app in App::ALL {
            let dir = workspace.path(app.mobile_dir());
            assert!(dir.join("README.md").is_file());
            for gradle in ["settings.gradle.kts", "settings.gradle", "build.gradle.kts"] {
                assert!(
                    !dir.join(gradle).exists(),
                    "{} now has a Gradle project: replace the UNSUPPORTED mobile surface with a real plan",
                    app.mobile_dir()
                );
            }
        }
    }
}
