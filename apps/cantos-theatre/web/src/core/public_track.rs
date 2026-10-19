//! Identity of one immutable public release, pinned from its manifest. It is never a title, a
//! display position, a mock fixture key or an expiring media URL, and holding one grants no
//! access by itself: the temporary capability is a separate, redacted value.
use super::public_ids::{canonical_uuid, PublicWorkId};
use cantos_contracts::theatre::{PlaybackAuthorization, PublicationManifest};
use std::fmt;
use uuid::Uuid;

/// Manifest schema this player understands.
const SCHEMA_VERSION: &str = "1";
/// The discovery contract bounds a verified rendition to `1..=3,600,000` ms.
pub const MAX_DURATION_MS: u64 = 3_600_000;
/// JSON's exact integer bound, shared with the other clients of this contract.
const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
const MANIFEST_DIGEST_PREFIX: &str = "rm-r1:sha256:";
const PROFILES: [&str; 4] = [
    "opus-mono-20-v1",
    "opus-mono-24-v1",
    "opus-mono-32-v1",
    "opus-mono-40-v1",
];
/// The server bounds a grant to 120 seconds.
pub const GRANT_TTL_MS: i64 = 120_000;
/// A listener's clock may run behind the server's; the upper bound is a sanity check only.
const CLOCK_SKEW_ALLOWANCE_MS: i64 = 300_000;
const MAX_MEDIA_URL_BYTES: usize = 8192;

/// A SHA-256 digest in the contract's lower-case 64-character hexadecimal form.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Sha256([u8; 32]);

impl Sha256 {
    pub fn parse(value: &str) -> Option<Self> {
        if value.len() != 64 {
            return None;
        }
        let mut bytes = [0u8; 32];
        let (pairs, _) = value.as_bytes().as_chunks::<2>();
        for (slot, pair) in bytes.iter_mut().zip(pairs) {
            *slot = (nibble(pair[0])? << 4) | nibble(pair[1])?;
        }
        Some(Self(bytes))
    }
}

fn nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

impl fmt::Display for Sha256 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0
            .iter()
            .try_for_each(|byte| write!(formatter, "{byte:02x}"))
    }
}

impl fmt::Debug for Sha256 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Sha256({self})")
    }
}

/// `rm-r1:sha256:<digest>`: the manifest's own immutable identity.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct ManifestDigest(Sha256);

impl ManifestDigest {
    pub fn parse(value: &str) -> Option<Self> {
        value
            .strip_prefix(MANIFEST_DIGEST_PREFIX)
            .and_then(Sha256::parse)
            .map(Self)
    }
}

impl fmt::Display for ManifestDigest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{MANIFEST_DIGEST_PREFIX}{}", self.0)
    }
}

impl fmt::Debug for ManifestDigest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "ManifestDigest({self})")
    }
}

/// What a listener asked to hear: one unit's exact release inside the Work being browsed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReleaseRequest {
    work: PublicWorkId,
    unit: Uuid,
    publication: Uuid,
}

impl ReleaseRequest {
    /// Both identifiers must be the contract's canonical lower-case form.
    pub fn parse(work: PublicWorkId, unit_id: &str, publication_id: &str) -> Option<Self> {
        Some(Self {
            work,
            unit: canonical_uuid(unit_id)?,
            publication: canonical_uuid(publication_id)?,
        })
    }
    pub const fn work(self) -> PublicWorkId {
        self.work
    }
    pub const fn unit(self) -> Uuid {
        self.unit
    }
    pub const fn publication(self) -> Uuid {
        self.publication
    }
}

/// The pinned identity of the audio being played. Only [`PinnedRelease::admit`] builds one, so a
/// value always came from a manifest that named the requested work, unit and publication.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PublicTrack {
    work: PublicWorkId,
    unit: Uuid,
    publication: Uuid,
    rendition: Uuid,
    manifest_digest: ManifestDigest,
    sha256: Sha256,
}

impl PublicTrack {
    pub const fn work(&self) -> PublicWorkId {
        self.work
    }
    pub const fn unit(&self) -> Uuid {
        self.unit
    }
    pub const fn publication(&self) -> Uuid {
        self.publication
    }
    pub const fn rendition(&self) -> Uuid {
        self.rendition
    }
    pub const fn manifest_digest(&self) -> ManifestDigest {
        self.manifest_digest
    }
    pub const fn sha256(&self) -> Sha256 {
        self.sha256
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ManifestRejection {
    /// Unknown schema, a container/codec/profile this player was not built for, or a bad size.
    Unsupported,
    /// The manifest names a different publication, unit or Work than the one requested.
    WrongRelease,
    /// A malformed identifier or digest.
    Malformed,
    /// A duration outside the contract's `1..=3,600,000` ms.
    Duration,
}

/// A manifest admitted for one request: the pinned identity and its verified duration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PinnedRelease {
    track: PublicTrack,
    duration_ms: u32,
}

impl PinnedRelease {
    pub fn admit(
        manifest: &PublicationManifest,
        requested: ReleaseRequest,
    ) -> Result<Self, ManifestRejection> {
        let publication =
            canonical_uuid(&manifest.publication_id).ok_or(ManifestRejection::Malformed)?;
        let work = PublicWorkId::parse(&manifest.work_id).ok_or(ManifestRejection::Malformed)?;
        let unit = canonical_uuid(&manifest.unit_id).ok_or(ManifestRejection::Malformed)?;
        let rendition =
            canonical_uuid(&manifest.rendition_id).ok_or(ManifestRejection::Malformed)?;
        let manifest_digest =
            ManifestDigest::parse(&manifest.manifest_digest).ok_or(ManifestRejection::Malformed)?;
        let sha256 = Sha256::parse(&manifest.sha256).ok_or(ManifestRejection::Malformed)?;
        if (work, unit, publication) != (requested.work, requested.unit, requested.publication) {
            return Err(ManifestRejection::WrongRelease);
        }
        let language = manifest.language.trim();
        let supported = manifest.schema_version == SCHEMA_VERSION
            && manifest.container == "ogg"
            && manifest.codec == "opus"
            && manifest.channels == 1
            && manifest.sample_rate_hz == 48_000
            && PROFILES.contains(&manifest.profile_id.as_str())
            && (1..=MAX_SAFE_INTEGER).contains(&manifest.byte_length)
            && u64::try_from(manifest.published_at_ms).is_ok_and(|at| at <= MAX_SAFE_INTEGER)
            && !language.is_empty()
            && language.len() <= 63;
        if !supported {
            return Err(ManifestRejection::Unsupported);
        }
        if !(1..=MAX_DURATION_MS).contains(&manifest.duration_ms) {
            return Err(ManifestRejection::Duration);
        }
        let duration_ms =
            u32::try_from(manifest.duration_ms).map_err(|_| ManifestRejection::Duration)?;
        Ok(Self {
            track: PublicTrack {
                work,
                unit,
                publication,
                rendition,
                manifest_digest,
                sha256,
            },
            duration_ms,
        })
    }
    pub const fn track(&self) -> PublicTrack {
        self.track
    }
    pub const fn duration_ms(&self) -> u32 {
        self.duration_ms
    }
}

/// A temporary bearer capability. It is played as given and never parsed, logged, persisted or
/// used as an identity, so its `Debug` form is redacted.
#[derive(Clone, PartialEq, Eq)]
pub struct MediaUrl(String);

impl MediaUrl {
    /// The exact string the audio element is given.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for MediaUrl {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("MediaUrl(<capability>)")
    }
}

/// HTTPS only, with no credentials, fragment, whitespace or control characters.
fn safe_media_url(value: &str) -> Option<MediaUrl> {
    if value.len() > MAX_MEDIA_URL_BYTES
        || value
            .bytes()
            .any(|byte| !byte.is_ascii_graphic() || byte == b'\\')
    {
        return None;
    }
    let parsed = url::Url::parse(value).ok()?;
    (parsed.scheme() == "https"
        && parsed.host_str().is_some()
        && parsed.username().is_empty()
        && parsed.password().is_none()
        && parsed.fragment().is_none())
    .then(|| MediaUrl(value.to_owned()))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GrantRejection {
    /// The authorization names another publication, rendition or SHA-256 than the pinned one.
    /// The player never switches to it.
    IdentityChanged,
    /// A body that breaks the contract: wrong media type, no byte ranges, an unsafe URL or an
    /// expiry that is already past or beyond the contract's window.
    Protocol,
}

/// An authorization proven to be for the pinned release, with the moment it stops working.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdmittedGrant {
    url: MediaUrl,
    expires_at_ms: i64,
}

impl AdmittedGrant {
    pub fn admit(
        track: &PublicTrack,
        authorization: &PlaybackAuthorization,
        now_ms: i64,
    ) -> Result<Self, GrantRejection> {
        let same_release = canonical_uuid(&authorization.publication_id) == Some(track.publication)
            && canonical_uuid(&authorization.rendition_id) == Some(track.rendition)
            && Sha256::parse(&authorization.sha256) == Some(track.sha256);
        if !same_release {
            return Err(GrantRejection::IdentityChanged);
        }
        let expires = authorization.expires_at_ms;
        let url = safe_media_url(&authorization.url);
        match url {
            Some(url)
                if authorization.content_type == "audio/ogg"
                    && authorization.byte_range_supported
                    && expires > now_ms
                    && expires - now_ms <= GRANT_TTL_MS + CLOCK_SKEW_ALLOWANCE_MS =>
            {
                Ok(Self {
                    url,
                    expires_at_ms: expires,
                })
            }
            _ => Err(GrantRejection::Protocol),
        }
    }
    pub fn url(&self) -> &MediaUrl {
        &self.url
    }
    pub const fn expires_at_ms(&self) -> i64 {
        self.expires_at_ms
    }
    /// A grant is only trusted for a request when it has not reached its expiry.
    pub const fn is_fresh_at(&self, now_ms: i64) -> bool {
        self.expires_at_ms > now_ms
    }
}

/// Distinct pinned releases for tests elsewhere in the crate; never compiled into the app.
#[cfg(test)]
pub(crate) mod testing {
    use super::*;

    /// A release whose every identifier derives from `n`, so two values never collide.
    pub(crate) fn release(n: u32) -> PinnedRelease {
        let uuid = |kind: u128| {
            Uuid::from_u128((kind << 100) | (4 << 76) | (0x8 << 62) | u128::from(n)).to_string()
        };
        let mut manifest: PublicationManifest = serde_json::from_str(include_str!(
            "../../../../../packages/contracts/fixtures/theatre/discovery/v1/manifest.json"
        ))
        .unwrap();
        manifest.work_id = uuid(1);
        manifest.unit_id = uuid(2);
        manifest.publication_id = uuid(8);
        manifest.rendition_id = uuid(6);
        manifest.sha256 = format!("{n:064x}");
        manifest.duration_ms = 600_000;
        let request = ReleaseRequest::parse(
            PublicWorkId::parse(&manifest.work_id).unwrap(),
            &manifest.unit_id,
            &manifest.publication_id,
        )
        .unwrap();
        PinnedRelease::admit(&manifest, request).unwrap()
    }

    pub(crate) fn source(n: u32) -> crate::core::playback::Source {
        let release = release(n);
        crate::core::playback::Source::public(release.track(), release.duration_ms()).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MANIFEST: &str = include_str!(
        "../../../../../packages/contracts/fixtures/theatre/discovery/v1/manifest.json"
    );
    const WORK: &str = "10000000-0000-4000-8000-000000000001";
    const UNIT: &str = "20000000-0000-4000-8000-000000000002";
    const PUBLICATION: &str = "80000000-0000-4000-8000-000000000008";
    const RENDITION: &str = "60000000-0000-4000-8000-000000000006";
    const SHA: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const URL: &str = "https://localhost:8443/api/v1/theatre/media/pg1_secret";

    fn manifest() -> PublicationManifest {
        serde_json::from_str(MANIFEST).unwrap()
    }
    fn request() -> ReleaseRequest {
        ReleaseRequest::parse(PublicWorkId::parse(WORK).unwrap(), UNIT, PUBLICATION).unwrap()
    }
    fn pinned() -> PinnedRelease {
        PinnedRelease::admit(&manifest(), request()).unwrap()
    }
    fn authorization() -> PlaybackAuthorization {
        PlaybackAuthorization {
            publication_id: PUBLICATION.into(),
            rendition_id: RENDITION.into(),
            sha256: SHA.into(),
            url: URL.into(),
            expires_at_ms: 1_000_000 + 120_000,
            content_type: "audio/ogg".into(),
            byte_range_supported: true,
        }
    }

    #[test]
    fn test_releases_are_distinct_and_valid_pins() {
        let (a, b) = (testing::release(1), testing::release(2));
        assert_ne!(a.track(), b.track());
        assert_ne!(a.track().publication(), b.track().publication());
        assert_eq!(testing::release(1), a);
        assert_eq!(a.duration_ms(), 600_000);
    }

    #[test]
    fn the_shared_manifest_fixture_pins_every_identity_and_the_verified_duration() {
        let release = pinned();
        let track = release.track();
        assert_eq!(track.work().to_string(), WORK);
        assert_eq!(track.unit().to_string(), UNIT);
        assert_eq!(track.publication().to_string(), PUBLICATION);
        assert_eq!(track.rendition().to_string(), RENDITION);
        assert_eq!(track.sha256().to_string(), SHA);
        assert_eq!(
            track.manifest_digest().to_string(),
            manifest().manifest_digest
        );
        assert_eq!(release.duration_ms(), 1000);
    }

    #[test]
    fn a_manifest_for_another_release_is_never_pinned_for_the_requested_one() {
        let mut other = manifest();
        other.publication_id = "80000000-0000-4000-8000-000000000009".into();
        assert_eq!(
            PinnedRelease::admit(&other, request()),
            Err(ManifestRejection::WrongRelease)
        );
        for edit in [
            (|m: &mut PublicationManifest| {
                m.unit_id = "20000000-0000-4000-8000-000000000003".into()
            }) as fn(&mut PublicationManifest),
            |m| m.work_id = "10000000-0000-4000-8000-000000000002".into(),
        ] {
            let mut changed = manifest();
            edit(&mut changed);
            assert_eq!(
                PinnedRelease::admit(&changed, request()),
                Err(ManifestRejection::WrongRelease)
            );
        }
    }

    #[test]
    fn malformed_unsupported_and_out_of_range_manifests_are_rejected_with_their_reason() {
        type Case = (
            &'static str,
            fn(&mut PublicationManifest),
            ManifestRejection,
        );
        let cases: [Case; 14] = [
            (
                "schema",
                |m| m.schema_version = "2".into(),
                ManifestRejection::Unsupported,
            ),
            (
                "container",
                |m| m.container = "mp3".into(),
                ManifestRejection::Unsupported,
            ),
            (
                "codec",
                |m| m.codec = "aac".into(),
                ManifestRejection::Unsupported,
            ),
            (
                "channels",
                |m| m.channels = 2,
                ManifestRejection::Unsupported,
            ),
            (
                "rate",
                |m| m.sample_rate_hz = 44_100,
                ManifestRejection::Unsupported,
            ),
            (
                "profile",
                |m| m.profile_id = "opus-mono-99-v1".into(),
                ManifestRejection::Unsupported,
            ),
            (
                "bytes",
                |m| m.byte_length = 0,
                ManifestRejection::Unsupported,
            ),
            (
                "language",
                |m| m.language = "  ".into(),
                ManifestRejection::Unsupported,
            ),
            (
                "digest",
                |m| m.manifest_digest = "sha256:aa".into(),
                ManifestRejection::Malformed,
            ),
            (
                "sha",
                |m| m.sha256 = "B".repeat(64),
                ManifestRejection::Malformed,
            ),
            (
                "rendition",
                |m| m.rendition_id = "RENDITION".into(),
                ManifestRejection::Malformed,
            ),
            (
                "zero duration",
                |m| m.duration_ms = 0,
                ManifestRejection::Duration,
            ),
            (
                "long duration",
                |m| m.duration_ms = MAX_DURATION_MS + 1,
                ManifestRejection::Duration,
            ),
            (
                "published",
                |m| m.published_at_ms = -1,
                ManifestRejection::Unsupported,
            ),
        ];
        for (name, edit, expected) in cases {
            let mut changed = manifest();
            edit(&mut changed);
            assert_eq!(
                PinnedRelease::admit(&changed, request()),
                Err(expected),
                "{name}"
            );
        }
        let mut longest = manifest();
        longest.duration_ms = MAX_DURATION_MS;
        assert_eq!(
            PinnedRelease::admit(&longest, request())
                .unwrap()
                .duration_ms(),
            3_600_000
        );
    }

    #[test]
    fn only_canonical_lower_case_digests_and_identifiers_are_admitted() {
        assert!(Sha256::parse(SHA).is_some());
        for rejected in [
            "",
            "b",
            &SHA[1..],
            &"B".repeat(64),
            &"g".repeat(64),
            &format!("{SHA}0"),
        ] {
            assert_eq!(Sha256::parse(rejected), None, "{rejected}");
        }
        assert_eq!(
            ReleaseRequest::parse(PublicWorkId::parse(WORK).unwrap(), "UNIT", PUBLICATION),
            None
        );
    }

    #[test]
    fn a_matching_authorization_is_admitted_and_its_url_is_redacted_everywhere() {
        let track = pinned().track();
        let grant = AdmittedGrant::admit(&track, &authorization(), 1_000_000).unwrap();
        assert_eq!(grant.url().as_str(), URL);
        assert_eq!(grant.expires_at_ms(), 1_120_000);
        assert!(grant.is_fresh_at(1_119_999) && !grant.is_fresh_at(1_120_000));
        let shown = format!("{grant:?} {:?} {track:?}", grant.url());
        assert!(
            !shown.contains("pg1_secret") && !shown.contains("localhost"),
            "{shown}"
        );
    }

    #[test]
    fn an_authorization_for_another_publication_rendition_or_sha_is_an_identity_change() {
        let track = pinned().track();
        let edits: [fn(&mut PlaybackAuthorization); 4] = [
            |a| a.publication_id = "80000000-0000-4000-8000-000000000009".into(),
            |a| a.rendition_id = "60000000-0000-4000-8000-000000000007".into(),
            |a| a.sha256 = "c".repeat(64),
            |a| a.sha256 = "not hex".into(),
        ];
        for edit in edits {
            let mut changed = authorization();
            edit(&mut changed);
            assert_eq!(
                AdmittedGrant::admit(&track, &changed, 1_000_000),
                Err(GrantRejection::IdentityChanged)
            );
        }
    }

    #[test]
    fn contract_violations_in_an_authorization_are_protocol_failures() {
        let track = pinned().track();
        type Edit = (&'static str, fn(&mut PlaybackAuthorization));
        let edits: [Edit; 10] = [
            ("content type", |a| a.content_type = "audio/mpeg".into()),
            ("no ranges", |a| a.byte_range_supported = false),
            ("http", |a| a.url = "http://localhost:8080/a".into()),
            ("credentials", |a| a.url = "https://u:p@localhost/a".into()),
            ("fragment", |a| a.url = format!("{URL}#t=1")),
            ("space", |a| a.url = format!("{URL} x")),
            ("backslash", |a| a.url = "https://localhost/a\\b".into()),
            ("expired", |a| a.expires_at_ms = 1_000_000),
            ("too far", |a| a.expires_at_ms = 1_000_000 + 600_000),
            ("empty", |a| a.url.clear()),
        ];
        for (name, edit) in edits {
            let mut changed = authorization();
            edit(&mut changed);
            assert_eq!(
                AdmittedGrant::admit(&track, &changed, 1_000_000),
                Err(GrantRejection::Protocol),
                "{name}"
            );
        }
        let mut oversized = authorization();
        oversized.url = format!("https://localhost/{}", "a".repeat(MAX_MEDIA_URL_BYTES));
        assert_eq!(
            AdmittedGrant::admit(&track, &oversized, 1_000_000),
            Err(GrantRejection::Protocol)
        );
    }

    #[test]
    fn clock_skew_is_tolerated_behind_the_server_but_never_admits_an_expired_grant() {
        let track = pinned().track();
        let server_now = 2_000_000;
        let mut issued = authorization();
        issued.expires_at_ms = server_now + GRANT_TTL_MS;
        // A device four minutes behind sees a six-minute grant; six minutes behind is not sane.
        assert!(AdmittedGrant::admit(&track, &issued, server_now - 240_000).is_ok());
        assert_eq!(
            AdmittedGrant::admit(&track, &issued, server_now - 360_000),
            Err(GrantRejection::Protocol)
        );
        // A device ahead of the server by the whole window sees an already expired grant.
        assert_eq!(
            AdmittedGrant::admit(&track, &issued, server_now + GRANT_TTL_MS),
            Err(GrantRejection::Protocol)
        );
    }
}
