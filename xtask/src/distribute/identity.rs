//! The build identity an operator-distributed development APK carries (#644).
//!
//! Gradle's `versionCode=1` / `0.1.0-dev` never changed between builds, so an installed APK
//! could not be tied to the source it was built from and Android/Firebase saw every upload as
//! the same version. A distribution now passes two explicit Gradle inputs: a strictly increasing
//! `versionCode` (Unix seconds at the run) and the source revision, which becomes build metadata
//! of `versionName`. The staged APK is checked against both before anything is uploaded, so a
//! stale or reused output cannot be distributed under a fresh label. Pure: facts are arguments.

/// Android's `versionCode` ceiling (`Integer.MAX_VALUE` minus the Play Store reserved range).
const MAX_VERSION_CODE: u64 = 2_100_000_000;
/// Leading hexadecimal digits of the commit kept in `versionName`.
const REVISION_LENGTH: usize = 12;
const DIRTY_MARK: &str = "-dirty";
/// The Gradle `debug` build type appends this suffix to `versionName`.
const DEVELOPMENT_SUFFIX: &str = "-dev";

pub const VERSION_CODE_PROPERTY: &str = "cantos.versionCode";
pub const SOURCE_REVISION_PROPERTY: &str = "cantos.sourceRevision";

/// What a dry-run shows in place of values that only exist at run time.
pub const PLAN_ARGS: [&str; 2] = [
    "-Pcantos.versionCode=<unix seconds at run>",
    "-Pcantos.sourceRevision=<HEAD short SHA>[-dirty]",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildIdentity {
    version_code: u32,
    revision: String,
}

impl BuildIdentity {
    pub fn new(unix_seconds: u64, commit: &str, dirty: bool) -> Result<Self, String> {
        let version_code = u32::try_from(unix_seconds)
            .ok()
            .filter(|_| (1..=MAX_VERSION_CODE).contains(&unix_seconds))
            .ok_or_else(|| {
                format!("clock value {unix_seconds} is not a usable Android versionCode")
            })?;
        let short: String = commit.chars().take(REVISION_LENGTH).collect();
        if short.len() < REVISION_LENGTH
            || !short
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err("the source commit is not a lower-case hexadecimal object name".to_owned());
        }
        let revision = if dirty {
            format!("{short}{DIRTY_MARK}")
        } else {
            short
        };
        Ok(Self {
            version_code,
            revision,
        })
    }

    pub fn version_code(&self) -> u32 {
        self.version_code
    }

    pub fn revision(&self) -> &str {
        &self.revision
    }

    pub fn gradle_args(&self) -> [String; 2] {
        [
            format!("-P{VERSION_CODE_PROPERTY}={}", self.version_code),
            format!("-P{SOURCE_REVISION_PROPERTY}={}", self.revision),
        ]
    }

    /// The staged APK must carry exactly this identity; anything else is an old or foreign output.
    pub fn verify(&self, version_code: &str, version_name: &str) -> Result<(), String> {
        let expected_suffix = format!("+{}{DEVELOPMENT_SUFFIX}", self.revision);
        if version_code != self.version_code.to_string()
            || !version_name.ends_with(&expected_suffix)
        {
            return Err(format!(
                "the APK reports versionName `{version_name}` ({version_code}) but this run built source `{}` as versionCode {}; the output is stale or was not produced by this run",
                self.revision, self.version_code
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";

    #[test]
    fn identity_is_the_clock_and_the_short_commit() {
        let identity = BuildIdentity::new(1_793_000_000, COMMIT, false).unwrap();
        assert_eq!(identity.version_code(), 1_793_000_000);
        assert_eq!(identity.revision(), "0123456789ab");
        assert_eq!(
            identity.gradle_args(),
            [
                "-Pcantos.versionCode=1793000000",
                "-Pcantos.sourceRevision=0123456789ab"
            ]
        );
    }

    #[test]
    fn an_uncommitted_tree_is_labelled_and_never_matches_the_clean_label() {
        let dirty = BuildIdentity::new(1_793_000_000, COMMIT, true).unwrap();
        let clean = BuildIdentity::new(1_793_000_000, COMMIT, false).unwrap();
        assert_eq!(dirty.revision(), "0123456789ab-dirty");
        assert!(dirty
            .verify("1793000000", "0.1.0+0123456789ab-dirty-dev")
            .is_ok());
        assert!(dirty
            .verify("1793000000", "0.1.0+0123456789ab-dev")
            .is_err());
        assert!(clean
            .verify("1793000000", "0.1.0+0123456789ab-dirty-dev")
            .is_err());
    }

    #[test]
    fn a_clock_outside_the_android_range_or_a_malformed_commit_is_refused() {
        for seconds in [0, MAX_VERSION_CODE + 1, u64::MAX] {
            assert!(BuildIdentity::new(seconds, COMMIT, false).is_err());
        }
        assert!(BuildIdentity::new(MAX_VERSION_CODE, COMMIT, false).is_ok());
        for commit in ["", "0123", "ABCDEF0123456789", "gggggggggggggggg"] {
            assert!(BuildIdentity::new(1, commit, false).is_err(), "{commit}");
        }
    }

    #[test]
    fn a_reused_output_with_the_old_constant_version_fails_verification() {
        let identity = BuildIdentity::new(1_793_000_000, COMMIT, false).unwrap();
        let old = identity.verify("1", "0.1.0-dev").unwrap_err();
        assert!(old.contains("stale") && old.contains("0123456789ab"));
        assert!(identity
            .verify("1793000001", "0.1.0+0123456789ab-dev")
            .is_err());
        assert!(identity
            .verify("1793000000", "0.1.0+fffffffffffff-dev")
            .is_err());
        assert!(identity
            .verify("1793000000", "0.1.0+0123456789ab-dev")
            .is_ok());
    }
}
