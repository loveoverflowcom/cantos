//! Accepted #157 P0 cover rule (ADR 0033): the public Work cover is the artwork effective at
//! the exact Work metadata revision pinned by the eligible representative release, disclosed
//! only by the current owner's explicit public-display decision for that exact image. Private
//! admission, Work visibility and audio publication approval are separate facts, never this one.
//! Pure and total: every cover failure is a stable fallback, never an error that would hide the
//! title or block listening. No production reader supplies a decision until #171 wires one.
use super::{
    artwork::{artwork_at, ArtworkChange, WorkArtworkSelection},
    image::{ImageAsset, ImageMime},
    listening::ListenerPublication,
};
use crate::contexts::catalog::domain::Work;

/// One reviewed public-display decision, bound to an exact immutable image asset digest.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoverDisclosure<'a> {
    /// No reviewed decision exists. Today this is the only value any shell can supply.
    Undecided,
    Cleared {
        image_asset_digest: &'a str,
        valid_until_ms: i64,
    },
    Revoked {
        image_asset_digest: &'a str,
    },
}

/// Facts the shell resolves for the release-pinned Work revision; none are looked up here.
/// Deliberately not Debug: the asset carries private provenance and rights references.
#[derive(Clone, Copy)]
pub struct CoverFacts<'a> {
    /// The Work's complete artwork history; changes after the pinned revision are ignored.
    pub changes: &'a [ArtworkChange],
    /// The asset named by the pinned selection, if the shell found one.
    pub asset: Option<&'a ImageAsset>,
    pub disclosure: CoverDisclosure<'a>,
    pub now_ms: i64,
}

/// Public projection only: no rights references, provenance, owner, intent, receipt or storage.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublicCover {
    image_asset_id: String,
    image_asset_digest: String,
    alt_text: String,
    content_type: ImageMime,
    width_px: u32,
    height_px: u32,
    byte_length: u64,
    sha256: String,
}
impl PublicCover {
    pub fn image_asset_id(&self) -> &str {
        &self.image_asset_id
    }
    pub fn image_asset_digest(&self) -> &str {
        &self.image_asset_digest
    }
    pub fn alt_text(&self) -> &str {
        &self.alt_text
    }
    pub const fn content_type(&self) -> ImageMime {
        self.content_type
    }
    pub const fn width_px(&self) -> u32 {
        self.width_px
    }
    pub const fn height_px(&self) -> u32 {
        self.height_px
    }
    pub const fn byte_length(&self) -> u64 {
        self.byte_length
    }
    pub fn sha256(&self) -> &str {
        &self.sha256
    }
}

/// Internal reason for a missing cover. Consumers render the same stable no-image fallback.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoverFallback {
    NoArtwork,
    Undisclosed,
    Revoked,
    Expired,
    Corrupt,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CoverProjection {
    Shown(PublicCover),
    Fallback(CoverFallback),
}

/// Only an eligible `ListenerPublication` can request a cover, so release eligibility,
/// current Work visibility and withdrawal are already decided before this rule runs.
pub fn project_public_cover(
    publication: &ListenerPublication,
    facts: CoverFacts<'_>,
) -> CoverProjection {
    project_at(publication.work(), facts)
}

/// Proposed metadata-only rule: the caller must first establish a current, source-verified
/// metadata disclosure and pass that exact immutable Work snapshot. Host policy and image
/// consent remain separate. This kernel neither reads nor grants either authority.
#[cfg(test)]
pub(crate) fn project_metadata_cover(
    disclosed: &Work,
    consent_revision: Option<u64>,
    facts: CoverFacts<'_>,
) -> CoverProjection {
    if consent_revision != Some(disclosed.revision()) {
        return CoverProjection::Fallback(CoverFallback::Undisclosed);
    }
    project_at(disclosed, facts)
}

pub(crate) fn project_at(pinned: &Work, facts: CoverFacts<'_>) -> CoverProjection {
    let bound = pinned_selection(pinned, facts.changes, facts.asset);
    let pinned_state = match &bound {
        Ok(Some(_)) => Pinned::Bound,
        Ok(None) => Pinned::NoArtwork,
        Err(()) => Pinned::Corrupt,
    };
    let digest = bound
        .as_ref()
        .ok()
        .and_then(Option::as_ref)
        .map(|(selection, _)| selection.image_asset_digest());
    let decision = match facts.disclosure {
        CoverDisclosure::Undecided => Decision::Undecided,
        CoverDisclosure::Cleared {
            image_asset_digest,
            valid_until_ms,
        } => Decision::Cleared {
            same_image: digest == Some(image_asset_digest),
            valid_until_ms,
        },
        CoverDisclosure::Revoked { image_asset_digest } => Decision::Revoked {
            same_image: digest == Some(image_asset_digest),
        },
    };
    match (cover_outcome(pinned_state, decision, facts.now_ms), bound) {
        (Ok(()), Ok(Some((selection, asset)))) => {
            let metadata = asset.metadata();
            if metadata.content_type() != ImageMime::Avif {
                return CoverProjection::Fallback(CoverFallback::Corrupt);
            }
            CoverProjection::Shown(PublicCover {
                image_asset_id: selection.image_asset_id().as_str().into(),
                image_asset_digest: selection.image_asset_digest().into(),
                alt_text: selection.alt_text().into(),
                content_type: metadata.content_type(),
                width_px: metadata.width_px(),
                height_px: metadata.height_px(),
                byte_length: metadata.byte_length(),
                sha256: metadata.sha256().into(),
            })
        }
        (Ok(()), _) => CoverProjection::Fallback(CoverFallback::Corrupt),
        (Err(fallback), _) => CoverProjection::Fallback(fallback),
    }
}

/// Delivery admission for `GET|HEAD /works/{work}/cover/{sha256}`: bytes are served only while
/// the freshly computed projection shows exactly that image. A stale URL, a fallback of any
/// reason and a malformed identity are indistinguishable denials.
pub fn admit_cover_fetch<'p>(
    projection: &'p CoverProjection,
    requested_sha256: &str,
) -> Option<&'p PublicCover> {
    match projection {
        CoverProjection::Shown(cover) if cover.sha256 == requested_sha256 => Some(cover),
        CoverProjection::Shown(_) | CoverProjection::Fallback(_) => None,
    }
}

/// The selection effective at the pinned revision, bound to its exact Work-owned asset.
fn pinned_selection<'a>(
    pinned: &Work,
    changes: &[ArtworkChange],
    asset: Option<&'a ImageAsset>,
) -> Result<Option<(WorkArtworkSelection, &'a ImageAsset)>, ()> {
    let response = artwork_at(pinned, changes).map_err(|_| ())?;
    let Some(raw) = response.artwork else {
        return Ok(None);
    };
    let selection = WorkArtworkSelection::restore(&raw).map_err(|_| ())?;
    let asset = asset.ok_or(())?;
    selection.require_asset(pinned, asset).map_err(|_| ())?;
    Ok(Some((selection, asset)))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Pinned {
    NoArtwork,
    Corrupt,
    Bound,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Decision {
    Undecided,
    Cleared {
        same_image: bool,
        valid_until_ms: i64,
    },
    Revoked {
        same_image: bool,
    },
}

/// Loop-free precedence kernel shared by the projection and its bounded proof. A decision
/// about another image is a binding defect, never consent; expiry is half-open.
fn cover_outcome(pinned: Pinned, decision: Decision, now_ms: i64) -> Result<(), CoverFallback> {
    match pinned {
        Pinned::Corrupt => return Err(CoverFallback::Corrupt),
        Pinned::NoArtwork => return Err(CoverFallback::NoArtwork),
        Pinned::Bound => {}
    }
    if now_ms < 0 {
        return Err(CoverFallback::Corrupt);
    }
    match decision {
        Decision::Undecided => Err(CoverFallback::Undisclosed),
        Decision::Cleared {
            same_image: false, ..
        }
        | Decision::Revoked { same_image: false } => Err(CoverFallback::Corrupt),
        Decision::Revoked { same_image: true } => Err(CoverFallback::Revoked),
        Decision::Cleared {
            same_image: true,
            valid_until_ms,
        } if now_ms >= valid_until_ms => Err(CoverFallback::Expired),
        Decision::Cleared {
            same_image: true, ..
        } => Ok(()),
    }
}

#[cfg(test)]
#[path = "public_cover_tests.rs"]
mod tests;

#[cfg(kani)]
mod kani_proofs {
    use super::{cover_outcome, CoverFallback, Decision, Pinned};
    /// Every pinned state, decision kind, digest relation and full i64 clock/expiry.
    /// Proves the exact disclosure precedence; not artwork history, SQL, storage or HTTP.
    #[kani::proof]
    #[kani::unwind(2)]
    fn public_cover_requires_pinned_bound_image_and_current_disclosure() {
        let pinned = match kani::any::<u8>() % 3 {
            0 => Pinned::NoArtwork,
            1 => Pinned::Corrupt,
            _ => Pinned::Bound,
        };
        let same_image: bool = kani::any();
        let valid_until: i64 = kani::any();
        let decision = match kani::any::<u8>() % 3 {
            0 => Decision::Undecided,
            1 => Decision::Cleared {
                same_image,
                valid_until_ms: valid_until,
            },
            _ => Decision::Revoked { same_image },
        };
        let now: i64 = kani::any();
        let outcome = cover_outcome(pinned, decision, now);
        let expected = if pinned == Pinned::Corrupt {
            Err(CoverFallback::Corrupt)
        } else if pinned == Pinned::NoArtwork {
            Err(CoverFallback::NoArtwork)
        } else if now < 0 {
            Err(CoverFallback::Corrupt)
        } else if decision == Decision::Undecided {
            Err(CoverFallback::Undisclosed)
        } else if !same_image {
            Err(CoverFallback::Corrupt)
        } else if decision == (Decision::Revoked { same_image: true }) {
            Err(CoverFallback::Revoked)
        } else if now >= valid_until {
            Err(CoverFallback::Expired)
        } else {
            Ok(())
        };
        assert_eq!(outcome, expected);
        // Shown implies a bound pinned image and a same-image clearance valid now.
        assert!(
            outcome.is_err()
                || (pinned == Pinned::Bound
                    && decision
                        == (Decision::Cleared {
                            same_image: true,
                            valid_until_ms: valid_until
                        })
                    && 0 <= now
                    && now < valid_until)
        );
        kani::cover!(outcome.is_ok(), "cleared pinned image shown");
        kani::cover!(
            outcome == Err(CoverFallback::Expired) && now == valid_until,
            "exclusive clearance expiry"
        );
        kani::cover!(
            outcome == Err(CoverFallback::Corrupt) && pinned == Pinned::Bound && !same_image,
            "decision for another image is not consent"
        );
        kani::cover!(
            outcome == Err(CoverFallback::Revoked),
            "revoked image hidden"
        );
        kani::cover!(
            outcome == Err(CoverFallback::Undisclosed),
            "undecided image hidden"
        );
        kani::cover!(
            outcome == Err(CoverFallback::NoArtwork),
            "cleared pin falls back"
        );
    }
}
