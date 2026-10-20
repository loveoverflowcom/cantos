//! Private trust anchors for one pinned identity-provider origin.
//!
//! Absent configuration keeps the built-in public roots. A configured bundle **replaces** them
//! for the clients that receive it, so a private issuer cannot be impersonated through a public
//! authority and no other client in the process gains an anchor. Certificate validation is
//! never disabled, and a bundle carrying anything but certificates is refused.
use std::{fs::File, io::Read, path::Path};

use reqwest::{Certificate, ClientBuilder};

use super::core::AuthError;

const MAX_BUNDLE_BYTES: u64 = 64 * 1024;
const MAX_ANCHORS: usize = 8;
const CERTIFICATE_BEGIN: &str = "-----BEGIN CERTIFICATE-----";

#[derive(Clone)]
pub struct IssuerTrust {
    anchors: Vec<Certificate>,
}

impl std::fmt::Debug for IssuerTrust {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "IssuerTrust({} anchors)", self.anchors.len())
    }
}

impl IssuerTrust {
    /// Pure parse of an operator-supplied PEM bundle. Every block must be a certificate: a
    /// private key, CRL or truncated block is a configuration error, never silently skipped.
    pub fn from_pem_bundle(pem: &[u8]) -> Result<Self, AuthError> {
        let text = std::str::from_utf8(pem).map_err(|_| AuthError::Configuration)?;
        if pem.len() as u64 > MAX_BUNDLE_BYTES {
            return Err(AuthError::Configuration);
        }
        let mut declared = 0;
        for line in text.lines().map(str::trim_end) {
            if line.starts_with("-----BEGIN ") {
                if line != CERTIFICATE_BEGIN {
                    return Err(AuthError::Configuration);
                }
                declared += 1;
            }
        }
        let anchors = Certificate::from_pem_bundle(pem).map_err(|_| AuthError::Configuration)?;
        if anchors.is_empty() || anchors.len() != declared || anchors.len() > MAX_ANCHORS {
            return Err(AuthError::Configuration);
        }
        Ok(Self { anchors })
    }

    /// Startup-only read of an existing operator file: absolute, regular, bounded and not
    /// writable by group/others. Nothing is created, fetched or installed into a system store.
    pub fn read_file(path: &Path) -> Result<Self, AuthError> {
        if !path.is_absolute() {
            return Err(AuthError::Configuration);
        }
        let file = File::open(path).map_err(|_| AuthError::Configuration)?;
        let metadata = file.metadata().map_err(|_| AuthError::Configuration)?;
        if !metadata.is_file() || metadata.len() > MAX_BUNDLE_BYTES || writable_by_others(&metadata)
        {
            return Err(AuthError::Configuration);
        }
        let mut bytes = Vec::new();
        file.take(MAX_BUNDLE_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| AuthError::Configuration)?;
        Self::from_pem_bundle(&bytes)
    }

    pub(crate) fn apply(&self, builder: ClientBuilder) -> ClientBuilder {
        self.anchors
            .iter()
            .cloned()
            .fold(builder.tls_built_in_root_certs(false), |builder, anchor| {
                builder.add_root_certificate(anchor)
            })
    }

    pub fn anchor_count(&self) -> usize {
        self.anchors.len()
    }
}

#[cfg(unix)]
fn writable_by_others(metadata: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    metadata.permissions().mode() & 0o022 != 0
}

#[cfg(not(unix))]
fn writable_by_others(_: &std::fs::Metadata) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    // Synthetic public CA certificates created only for these tests; their keys were discarded.
    const CA_A: &str = "-----BEGIN CERTIFICATE-----
MIIB3zCCAYWgAwIBAgIUcUf+FotuIYTa7tBlIcAUtxto/n4wCgYIKoZIzj0EAwIw
PDE6MDgGA1UEAwwxQ2FudG9zIHN5bnRoZXRpYyB0ZXN0IENBIGEgKG5vdCB0cnVz
dGVkIGFueXdoZXJlKTAgFw0yNjEwMTYxMDUzNTlaGA8yMTI2MDkyMjEwNTM1OVow
PDE6MDgGA1UEAwwxQ2FudG9zIHN5bnRoZXRpYyB0ZXN0IENBIGEgKG5vdCB0cnVz
dGVkIGFueXdoZXJlKTBZMBMGByqGSM49AgEGCCqGSM49AwEHA0IABA9VAS+LIm3L
Wwm4AZCB1MXqs7zhDl94wFWIRKmhyS+VupqxTWf7BoPnz0NxVcGKl9LMP9z02Oc+
QU/cI/C0yNGjYzBhMB0GA1UdDgQWBBTE2MJ7AzhlrVMy/t1aT9a6Jq/YbjAfBgNV
HSMEGDAWgBTE2MJ7AzhlrVMy/t1aT9a6Jq/YbjAPBgNVHRMBAf8EBTADAQH/MA4G
A1UdDwEB/wQEAwIBBjAKBggqhkjOPQQDAgNIADBFAiEA9WNCFPLqwAWe16BDVmJV
nGI7o/5WBVXUPpN1unr/NRcCIGg7QqmeePEsZQbJcJNPFwqKaFIpEjpwkguhmyc4
p6ER
-----END CERTIFICATE-----
";
    const CA_B: &str = "-----BEGIN CERTIFICATE-----
MIIB3jCCAYWgAwIBAgIUXGhtQoRLYA/GxR6xw1xA7716q18wCgYIKoZIzj0EAwIw
PDE6MDgGA1UEAwwxQ2FudG9zIHN5bnRoZXRpYyB0ZXN0IENBIGIgKG5vdCB0cnVz
dGVkIGFueXdoZXJlKTAgFw0yNjEwMTYxMDUzNTlaGA8yMTI2MDkyMjEwNTM1OVow
PDE6MDgGA1UEAwwxQ2FudG9zIHN5bnRoZXRpYyB0ZXN0IENBIGIgKG5vdCB0cnVz
dGVkIGFueXdoZXJlKTBZMBMGByqGSM49AgEGCCqGSM49AwEHA0IABNk2V0VfZLzK
oVa3hLEq4DwoPrgKJIBBqRAyuePaaH+6ayndeNAHUBFzNWLwPhUGCSqZSUZ69Tkc
7/hUYCmRl9ijYzBhMB0GA1UdDgQWBBS6LRXQQGInWpd9eL4IpOWwYqlv2zAfBgNV
HSMEGDAWgBS6LRXQQGInWpd9eL4IpOWwYqlv2zAPBgNVHRMBAf8EBTADAQH/MA4G
A1UdDwEB/wQEAwIBBjAKBggqhkjOPQQDAgNHADBEAiA1rMjk+U9xoXQ6NRTvmzc0
ESdjyeZSiPQWQfOtoS9TqwIgaMC1XNajeRcZHldJlW4+FAFT3XC6F299H9yRqaxX
XmU=
-----END CERTIFICATE-----
";

    #[test]
    fn one_or_more_certificates_become_exactly_that_many_anchors() {
        assert_eq!(
            IssuerTrust::from_pem_bundle(CA_A.as_bytes())
                .unwrap()
                .anchor_count(),
            1
        );
        let both = format!("{CA_A}\n{CA_B}");
        let trust = IssuerTrust::from_pem_bundle(both.as_bytes()).unwrap();
        assert_eq!(trust.anchor_count(), 2);
        assert_eq!(format!("{trust:?}"), "IssuerTrust(2 anchors)");
    }

    #[test]
    fn a_bundle_that_is_not_purely_certificates_is_refused_with_the_configuration_error() {
        let key_block = "-----BEGIN PRIVATE KEY-----\nAAAA\n-----END PRIVATE KEY-----\n";
        let truncated = CA_A.replace("-----END CERTIFICATE-----\n", "");
        let bad_base64 = CA_A.replace("MIIB3zCC", "!!!!!!!!");
        for (name, bundle) in [
            ("empty", String::new()),
            ("text only", "not a certificate".to_owned()),
            ("private key only", key_block.to_owned()),
            ("certificate plus private key", format!("{CA_A}{key_block}")),
            ("truncated block", truncated),
            ("undecodable block", bad_base64),
            ("too many anchors", CA_A.repeat(MAX_ANCHORS + 1)),
        ] {
            assert_eq!(
                IssuerTrust::from_pem_bundle(bundle.as_bytes()).err(),
                Some(AuthError::Configuration),
                "{name}"
            );
        }
        assert_eq!(
            IssuerTrust::from_pem_bundle(&[0xff, 0xfe]).err(),
            Some(AuthError::Configuration)
        );
        let oversized = format!("{CA_A}{}", "#".repeat(MAX_BUNDLE_BYTES as usize));
        assert_eq!(
            IssuerTrust::from_pem_bundle(oversized.as_bytes()).err(),
            Some(AuthError::Configuration)
        );
    }

    #[test]
    fn applying_anchors_builds_a_client_without_enabling_any_bypass() {
        let trust = IssuerTrust::from_pem_bundle(CA_A.as_bytes()).unwrap();
        assert!(trust.apply(reqwest::Client::builder()).build().is_ok());
    }

    #[test]
    fn private_anchors_are_accepted_only_for_an_https_issuer() {
        use crate::platform::oidc::config::OidcConfig;
        const PATH: &str = "/oauth2/openid/cantos-theatre-web-bff";
        let trust = IssuerTrust::from_pem_bundle(CA_A.as_bytes()).unwrap();
        let loopback = OidcConfig::theatre_web(
            &format!("http://127.0.0.1:8443{PATH}"),
            "http://127.0.0.1:8082",
            "secret".into(),
        )
        .unwrap();
        assert_eq!(
            loopback.with_trust(trust.clone()).err(),
            Some(AuthError::Configuration)
        );
        let private = OidcConfig::theatre_web(
            &format!("https://idm.localhost:8444{PATH}"),
            "https://localhost:8443",
            "secret".into(),
        )
        .unwrap();
        assert_eq!(
            private
                .with_trust(trust)
                .unwrap()
                .trust()
                .unwrap()
                .anchor_count(),
            1
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_file_must_be_absolute_regular_bounded_and_not_writable_by_others() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("cantos-trust-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let write = |name: &str, body: &str, mode: u32| {
            let path = dir.join(name);
            std::fs::write(&path, body).unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode)).unwrap();
            path
        };
        let good = write("good.pem", CA_A, 0o600);
        assert_eq!(IssuerTrust::read_file(&good).unwrap().anchor_count(), 1);
        let loose = write("loose.pem", CA_A, 0o666);
        let garbage = write("garbage.pem", "nope", 0o600);
        let big = write("big.pem", &" ".repeat(MAX_BUNDLE_BYTES as usize + 1), 0o600);
        for bad in [
            Path::new("relative.pem"),
            dir.as_path(),
            dir.join("missing.pem").as_path(),
            loose.as_path(),
            garbage.as_path(),
            big.as_path(),
        ] {
            assert_eq!(
                IssuerTrust::read_file(bad).err(),
                Some(AuthError::Configuration),
                "{bad:?}"
            );
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
