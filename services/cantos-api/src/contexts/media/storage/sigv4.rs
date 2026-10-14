//! Narrow S3 SigV4 signing. Only this transport boundary handles credentials and signed URLs.
//! Canonicalization follows the official S3 reference vectors; it never defines asset identity.

use std::collections::BTreeMap;

use hmac::{Hmac, Mac};
use reqwest::Url;
use sha2::{Digest, Sha256};

use crate::contexts::media::domain::object_storage::StorageError;

pub(super) const EMPTY_PAYLOAD_SHA256: &str =
    "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

#[derive(Clone)]
pub(super) struct Signer {
    region: String,
    access_key: String,
    secret_key: String,
    session_token: Option<String>,
}

impl std::fmt::Debug for Signer {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Signer([redacted])")
    }
}

impl Signer {
    pub(super) fn new(
        region: String,
        access_key: String,
        secret_key: String,
        session_token: Option<String>,
    ) -> Result<Self, StorageError> {
        if region.is_empty()
            || region.len() > 63
            || !region.as_bytes()[0].is_ascii_alphanumeric()
            || !region.as_bytes()[region.len() - 1].is_ascii_alphanumeric()
            || !region
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
            || access_key.is_empty()
            || access_key.len() > 256
            || !access_key
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
            || !valid_secret(&secret_key, 4096)
            || session_token
                .as_deref()
                .is_some_and(|token| !valid_secret(token, 16384))
        {
            return Err(StorageError::InvalidTarget);
        }
        Ok(Self {
            region,
            access_key,
            secret_key,
            session_token,
        })
    }

    pub(super) fn headers(
        &self,
        method: &str,
        url: &Url,
        payload_hash: &str,
        timestamp: &str,
        additional: &[(&str, String)],
    ) -> Result<BTreeMap<String, String>, StorageError> {
        let mut headers = BTreeMap::from([
            ("host".to_owned(), authority(url)?),
            ("x-amz-content-sha256".to_owned(), payload_hash.to_owned()),
            ("x-amz-date".to_owned(), timestamp.to_owned()),
        ]);
        if let Some(token) = &self.session_token {
            headers.insert("x-amz-security-token".to_owned(), token.clone());
        }
        for (name, value) in additional {
            let name = name.to_ascii_lowercase();
            if headers.insert(name, normalize_header(value)).is_some() {
                return Err(StorageError::InvalidTarget);
            }
        }
        let signed_headers = headers.keys().cloned().collect::<Vec<_>>().join(";");
        let canonical_headers = canonical_headers(&headers);
        let query = canonical_query(
            url.query_pairs()
                .map(|(key, value)| (key.into_owned(), value.into_owned())),
        );
        let canonical_request = format!(
            "{method}\n{}\n{query}\n{canonical_headers}\n{signed_headers}\n{payload_hash}",
            canonical_path(url.path())?
        );
        let (scope, signature) = self.signature(timestamp, &canonical_request)?;
        headers.insert(
            "authorization".to_owned(),
            format!(
                "AWS4-HMAC-SHA256 Credential={}/{scope},SignedHeaders={signed_headers},Signature={signature}",
                self.access_key
            ),
        );
        Ok(headers)
    }

    /// Runtime callers impose the narrower 300-second grant bound. The primitive allows the
    /// official 24-hour golden vector without changing production grant policy for a test.
    pub(super) fn presign_get(
        &self,
        mut url: Url,
        timestamp: &str,
        ttl_seconds: u32,
        version_id: Option<&str>,
    ) -> Result<Url, StorageError> {
        if !(1..=604800).contains(&ttl_seconds) || url.query().is_some() {
            return Err(StorageError::InvalidTarget);
        }
        let date = signing_date(timestamp)?;
        let scope = format!("{date}/{}/s3/aws4_request", self.region);
        let mut parameters = vec![
            ("X-Amz-Algorithm".to_owned(), "AWS4-HMAC-SHA256".to_owned()),
            (
                "X-Amz-Credential".to_owned(),
                format!("{}/{scope}", self.access_key),
            ),
            ("X-Amz-Date".to_owned(), timestamp.to_owned()),
            ("X-Amz-Expires".to_owned(), ttl_seconds.to_string()),
            ("X-Amz-SignedHeaders".to_owned(), "host".to_owned()),
        ];
        if let Some(token) = &self.session_token {
            parameters.push(("X-Amz-Security-Token".to_owned(), token.clone()));
        }
        if let Some(version_id) = version_id {
            parameters.push(("versionId".to_owned(), version_id.to_owned()));
        }
        let query = canonical_query(parameters);
        let canonical_request = format!(
            "GET\n{}\n{query}\nhost:{}\n\nhost\nUNSIGNED-PAYLOAD",
            canonical_path(url.path())?,
            authority(&url)?
        );
        let (_, signature) = self.signature(timestamp, &canonical_request)?;
        url.set_query(Some(&format!("{query}&X-Amz-Signature={signature}")));
        Ok(url)
    }

    fn signature(
        &self,
        timestamp: &str,
        canonical_request: &str,
    ) -> Result<(String, String), StorageError> {
        let date = signing_date(timestamp)?;
        let scope = format!("{date}/{}/s3/aws4_request", self.region);
        let canonical_hash = format!("{:x}", Sha256::digest(canonical_request.as_bytes()));
        let string_to_sign = format!("AWS4-HMAC-SHA256\n{timestamp}\n{scope}\n{canonical_hash}");
        let date_key = hmac(
            format!("AWS4{}", self.secret_key).as_bytes(),
            date.as_bytes(),
        )?;
        let region_key = hmac(&date_key, self.region.as_bytes())?;
        let service_key = hmac(&region_key, b"s3")?;
        let signing_key = hmac(&service_key, b"aws4_request")?;
        let signature = hmac(&signing_key, string_to_sign.as_bytes())?;
        Ok((scope, hex(&signature)))
    }
}

fn valid_secret(value: &str, maximum: usize) -> bool {
    !value.is_empty()
        && value.len() <= maximum
        && value.bytes().all(|byte| (b'!'..=b'~').contains(&byte))
}

fn signing_date(timestamp: &str) -> Result<&str, StorageError> {
    let bytes = timestamp.as_bytes();
    if bytes.len() != 16
        || bytes[8] != b'T'
        || bytes[15] != b'Z'
        || !bytes[..8].iter().all(u8::is_ascii_digit)
        || !bytes[9..15].iter().all(u8::is_ascii_digit)
    {
        return Err(StorageError::InvalidTarget);
    }
    Ok(&timestamp[..8])
}

fn authority(url: &Url) -> Result<String, StorageError> {
    let host = url.host_str().ok_or(StorageError::InvalidTarget)?;
    let host = if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]")
    } else {
        host.to_owned()
    };
    Ok(match url.port() {
        Some(port) => format!("{host}:{port}"),
        None => host,
    })
}

fn normalize_header(value: &str) -> String {
    value.split_ascii_whitespace().collect::<Vec<_>>().join(" ")
}

fn canonical_headers(headers: &BTreeMap<String, String>) -> String {
    headers
        .iter()
        .map(|(name, value)| format!("{name}:{}\n", normalize_header(value)))
        .collect()
}

pub(super) fn canonical_query(parameters: impl IntoIterator<Item = (String, String)>) -> String {
    let mut parameters = parameters
        .into_iter()
        .map(|(name, value)| (uri_encode(name.as_bytes()), uri_encode(value.as_bytes())))
        .collect::<Vec<_>>();
    parameters.sort();
    parameters
        .into_iter()
        .map(|(name, value)| format!("{name}={value}"))
        .collect::<Vec<_>>()
        .join("&")
}

fn canonical_path(path: &str) -> Result<String, StorageError> {
    let bytes = path.as_bytes();
    let mut result = String::new();
    let mut position = 0;
    while position < bytes.len() {
        let byte = bytes[position];
        if byte == b'%' {
            let escaped = bytes
                .get(position + 1..position + 3)
                .ok_or(StorageError::InvalidTarget)?;
            if !escaped.iter().all(u8::is_ascii_hexdigit) {
                return Err(StorageError::InvalidTarget);
            }
            result.push('%');
            result.push(char::from(escaped[0]).to_ascii_uppercase());
            result.push(char::from(escaped[1]).to_ascii_uppercase());
            position += 3;
        } else {
            if byte == b'/' {
                result.push('/');
            } else {
                result.push_str(&uri_encode(&[byte]));
            }
            position += 1;
        }
    }
    Ok(result)
}

fn uri_encode(bytes: &[u8]) -> String {
    let mut result = String::new();
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    for &byte in bytes {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            result.push(char::from(byte));
        } else {
            result.push('%');
            result.push(char::from(HEX[usize::from(byte >> 4)]));
            result.push(char::from(HEX[usize::from(byte & 15)]));
        }
    }
    result
}

fn hmac(key: &[u8], value: &[u8]) -> Result<[u8; 32], StorageError> {
    let mut mac = Hmac::<Sha256>::new_from_slice(key).map_err(|_| StorageError::Unavailable)?;
    mac.update(value);
    Ok(mac.finalize().into_bytes().into())
}

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        result.push(char::from(HEX[usize::from(byte >> 4)]));
        result.push(char::from(HEX[usize::from(byte & 15)]));
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reference_signer() -> Signer {
        // Public, deliberately fake credentials from the official AWS S3 signing examples.
        Signer::new(
            "us-east-1".to_owned(),
            "AKIAIOSFODNN7EXAMPLE".to_owned(),
            "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY".to_owned(),
            None,
        )
        .unwrap()
    }

    #[test]
    fn authorization_matches_official_aws_s3_get_range_vector() {
        // https://docs.aws.amazon.com/AmazonS3/latest/developerguide/sig-v4-header-based-auth.html
        let url = Url::parse("https://examplebucket.s3.amazonaws.com/test.txt").unwrap();
        let headers = reference_signer()
            .headers(
                "GET",
                &url,
                EMPTY_PAYLOAD_SHA256,
                "20130524T000000Z",
                &[("range", "bytes=0-9".to_owned())],
            )
            .unwrap();
        assert_eq!(
            headers["authorization"],
            concat!(
            "AWS4-HMAC-SHA256 Credential=AKIAIOSFODNN7EXAMPLE/20130524/us-east-1/s3/aws4_request,",
            "SignedHeaders=host;range;x-amz-content-sha256;x-amz-date,",
            "Signature=f0e8bdb87c964420e857bd35b5d6ed310bd44f0170aba48dd91039c6036bdb41"
        )
        );
    }

    #[test]
    fn authorization_matches_official_aws_s3_put_escaped_path_vector() {
        let url = Url::parse("https://examplebucket.s3.amazonaws.com/test%24file.text").unwrap();
        let headers = reference_signer()
            .headers(
                "PUT",
                &url,
                "44ce7dd67c959e0d3524ffac1771dfbba87d2b6b4b4e99e42034a8b803f8b072",
                "20130524T000000Z",
                &[
                    ("date", "Fri, 24 May 2013 00:00:00 GMT".to_owned()),
                    ("x-amz-storage-class", "REDUCED_REDUNDANCY".to_owned()),
                ],
            )
            .unwrap();
        assert!(headers["authorization"].ends_with(
            "Signature=98ad721746da40c64f1a55b78f14c238d841ea1380cd77a1b5971af0ece108bd"
        ));
    }

    #[test]
    fn presign_matches_official_aws_s3_query_vector() {
        // https://docs.aws.amazon.com/AmazonS3/latest/developerguide/sigv4-query-string-auth.html
        let url = Url::parse("https://examplebucket.s3.amazonaws.com/test.txt").unwrap();
        let signed = reference_signer()
            .presign_get(url, "20130524T000000Z", 86400, None)
            .unwrap();
        assert_eq!(
            signed.as_str(),
            concat!(
                "https://examplebucket.s3.amazonaws.com/test.txt?",
                "X-Amz-Algorithm=AWS4-HMAC-SHA256&",
                "X-Amz-Credential=AKIAIOSFODNN7EXAMPLE%2F20130524%2Fus-east-1%2Fs3%2Faws4_request&",
                "X-Amz-Date=20130524T000000Z&X-Amz-Expires=86400&X-Amz-SignedHeaders=host&",
                "X-Amz-Signature=aeeed9bbccd4d02ee5c0109b86d86835f995330da4c265957d157751f604d404"
            )
        );
    }

    #[test]
    fn query_encoding_preserves_exact_version_and_session_token() {
        let signer = Signer::new(
            "auto".to_owned(),
            "EXAMPLE".to_owned(),
            "secret-sentinel".to_owned(),
            Some("token/with+reserved=bytes".to_owned()),
        )
        .unwrap();
        let signed = signer
            .presign_get(
                Url::parse("https://bucket.example.com/private/audio/abc").unwrap(),
                "20130524T000000Z",
                300,
                Some("version/with+reserved=bytes"),
            )
            .unwrap();
        let parameters: BTreeMap<_, _> = signed.query_pairs().collect();
        assert_eq!(parameters["versionId"], "version/with+reserved=bytes");
        assert_eq!(
            parameters["X-Amz-Security-Token"],
            "token/with+reserved=bytes"
        );
        assert!(signed
            .as_str()
            .contains("versionId=version%2Fwith%2Breserved%3Dbytes"));
        assert!(!format!("{signer:?}").contains("sentinel"));
        assert!(!format!("{signer:?}").contains("EXAMPLE"));
        assert_eq!(
            canonical_path("/a//b%20c/$"),
            Ok("/a//b%20c/%24".to_owned())
        );
        assert_eq!(
            canonical_query([
                ("b".to_owned(), " +".to_owned()),
                ("a".to_owned(), "/".to_owned())
            ]),
            "a=%2F&b=%20%2B"
        );
    }

    #[test]
    fn invalid_credentials_timestamps_and_duplicate_headers_fail_closed() {
        for secret in ["", "secret\n", "secret with spaces"] {
            assert!(matches!(
                Signer::new(
                    "auto".to_owned(),
                    "EXAMPLE".to_owned(),
                    secret.to_owned(),
                    None
                ),
                Err(StorageError::InvalidTarget)
            ));
        }
        for timestamp in ["", "20130524", "20130524T000000Z\n", "20130524X000000Z"] {
            assert_eq!(signing_date(timestamp), Err(StorageError::InvalidTarget));
        }
        let url = Url::parse("https://examplebucket.s3.amazonaws.com/test.txt").unwrap();
        assert_eq!(
            reference_signer().headers(
                "GET",
                &url,
                EMPTY_PAYLOAD_SHA256,
                "20130524T000000Z",
                &[("host", "evil.example".to_owned())]
            ),
            Err(StorageError::InvalidTarget)
        );
        assert_eq!(
            reference_signer().presign_get(url, "20130524T000000Z", 0, None),
            Err(StorageError::InvalidTarget)
        );
    }
}
