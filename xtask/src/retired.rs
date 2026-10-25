//! Retired command surface. Cantos is native-only: PostgreSQL, Kanidm and the legacy VPS
//! runtime commands are disabled in this checkpoint. Legacy source still awaits retirement. They stay parseable
//! (hidden from `--help`) only so that an old muscle-memory invocation gets a precise reason
//! and the replacement instead of a generic "unrecognized subcommand". Every one of them
//! refuses as a usage error (exit 2) before any tool probe, file read, subprocess or write.
//! Git history preserves removed behavior; legacy test oracles are retained separately.

use clap::Args;

use crate::build::Refusal;

/// Swallows whatever the old invocation carried, so flags never turn the refusal into a
/// parse error.
#[derive(Debug, Clone, Default, Args, PartialEq, Eq)]
pub struct Retired {
    #[arg(
        trailing_var_arg = true,
        allow_hyphen_values = true,
        hide = true,
        num_args = 0..
    )]
    pub args: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Retirement {
    Kanidm,
    ServicesLifecycle,
    VerifyPostgres,
    VerifyDbsp,
    VerifyTheatreAuth,
    DeployVps,
    LegacyApiRelease,
    SeedEnvironment,
    RunTestCluster,
    BuildDbsp,
}

impl Retirement {
    pub fn name(self) -> &'static str {
        match self {
            Self::Kanidm => "kanidm",
            Self::ServicesLifecycle => "services up|down|logs|status",
            Self::VerifyPostgres => "verify postgres",
            Self::VerifyDbsp => "verify dbsp",
            Self::VerifyTheatreAuth => "verify theatre-auth",
            Self::DeployVps => "deploy vps",
            Self::LegacyApiRelease => "release-build api / release --component api",
            Self::SeedEnvironment => "seed theatre --environment",
            Self::RunTestCluster => "run --test-cluster",
            Self::BuildDbsp => "build --dbsp-experimental",
        }
    }

    fn reason(self) -> &'static str {
        match self {
            Self::Kanidm => "Kanidm is no longer part of Cantos. Identity is the native `cantos-identity` process with its own Turso file; there is no provider issuer, OAuth client or OIDC metadata to plan or check.",
            Self::ServicesLifecycle => "no container-managed development service remains (PostgreSQL and Kanidm were the only ones). The native composition is started by `cargo xtask run`; `cargo xtask services list|validate` still describe and check the inventory.",
            Self::VerifyPostgres => "there is no PostgreSQL gate: the Server persists in its own embedded Turso file and its foundation/session tests are ordinary file-backed tests inside `cargo xtask verify rust`; retained PostgreSQL schema and operator assets are recovery inputs. `cargo xtask verify turso` covers the shared store and Identity persistence.",
            Self::VerifyDbsp => "DBSP and its gate were retired. The current read-performance decision defers incremental engines: optimize measured Turso SQL/index first, then evaluate selective cache (issue #384); the `dbsp-experimental` feature no longer exists.",
            Self::VerifyTheatreAuth => "both lanes (Firebase Auth emulator enrollment, Kanidm web end-to-end) exercised provider authentication that was removed from the Server. Theatre identity uses Turso through the native `cantos-identity` process; qualify it with `cargo xtask verify native-auth-qualification`.",
            Self::DeployVps => "the legacy archive executor existed only for the PostgreSQL/Kanidm/two-API runtime (its execution path is disabled; legacy assets remain as migration inputs). The replacement is the native OCI release flow: `cargo xtask release oci-build` and `release oci-verify` (docs/deployment/native-oci-release.md). The host-local `release native-upgrade` lane only upgrades an already admitted NATIVE_READY host with the same schema/config; first-native or legacy cutover is not implemented.",
            Self::LegacyApiRelease => "the `api` release component was the legacy VPS archive (a `localhost/cantos-api` image tarball plus an `infra/vps` bundle for the PostgreSQL/Kanidm runtime). The Web and Android components are unchanged; the native Server/Identity image pair is built by `cargo xtask release oci-build` and checked by `release oci-verify`.",
            Self::SeedEnvironment => "a named persistent environment bound the Studio session to an actor and fence read from an externally reachable application database. A Turso file has one owner process, so no other process can read it while the Server runs and that binding cannot hold. Seeding is the local development composition only: `cargo xtask seed theatre`.",
            Self::RunTestCluster => "there is no retained disposable PostgreSQL cluster. `cargo xtask run` starts a native Server and Identity over private development files by itself.",
            Self::BuildDbsp => "the `dbsp-experimental` feature no longer exists; use `cargo xtask build cantos-server` for the Turso-backed Server.",
        }
    }

    pub fn refusal(self) -> Refusal {
        Refusal::Usage(format!("`{}` is retired: {}", self.name(), self.reason()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [Retirement; 10] = [
        Retirement::Kanidm,
        Retirement::ServicesLifecycle,
        Retirement::VerifyPostgres,
        Retirement::VerifyDbsp,
        Retirement::VerifyTheatreAuth,
        Retirement::DeployVps,
        Retirement::LegacyApiRelease,
        Retirement::SeedEnvironment,
        Retirement::RunTestCluster,
        Retirement::BuildDbsp,
    ];

    #[test]
    fn every_retirement_is_a_usage_refusal_that_names_the_command_and_a_replacement() {
        for retirement in ALL {
            let Refusal::Usage(message) = retirement.refusal() else {
                panic!("{retirement:?} must refuse as a usage error, never as UNSUPPORTED");
            };
            assert!(message.contains(retirement.name()), "{message}");
            assert!(message.contains("retired"), "{message}");
            assert!(
                message.contains("cargo xtask") || message.contains("Turso"),
                "{retirement:?} names no replacement: {message}"
            );
        }
    }

    #[test]
    fn the_vps_refusal_points_at_native_upgrade_and_refuses_first_native_cutover() {
        let Refusal::Usage(message) = Retirement::DeployVps.refusal() else {
            unreachable!()
        };
        assert!(message.contains("release oci-build"));
        assert!(message.contains("docs/deployment/native-oci-release.md"));
        assert!(message.contains("already admitted NATIVE_READY"));
        assert!(message.contains("same schema/config"));
        assert!(message.contains("first-native or legacy cutover is not implemented"));
    }
}
