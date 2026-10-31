//! Catalog-owned HIDE predicate for the public-only Library projection. Listening owns account
//! records, never the metadata/access tables. The outer account row is aliased `x`.
use crate::platform::store::WORK_OWNER_ACTIVE_FILTER;

pub(crate) fn library_visibility(unit: bool) -> String {
    let child = if unit {
        "AND EXISTS(SELECT 1 FROM catalog_metadata_generation_entries ge JOIN catalog_units gu ON gu.id=ge.unit_id AND gu.work_id=ge.work_id WHERE ge.work_id=p.work_id AND ge.generation=p.generation AND ge.unit_id=x.unit_id AND gu.lifecycle<>'archived' AND NOT EXISTS(SELECT 1 FROM catalog_unit_viewer_policies uv WHERE uv.unit_id=ge.unit_id AND uv.mode<>'inherit'))"
    } else {
        ""
    };
    format!("EXISTS(SELECT 1 FROM catalog_metadata_publication_pointers p JOIN catalog_works w ON w.id=p.work_id WHERE p.work_id=x.work_id AND p.lifecycle='active' AND w.lifecycle<>'archived' AND {WORK_OWNER_ACTIVE_FILTER} AND NOT EXISTS(SELECT 1 FROM catalog_viewer_policies vp WHERE vp.work_id=p.work_id AND vp.audience<>'public') {child})")
}
