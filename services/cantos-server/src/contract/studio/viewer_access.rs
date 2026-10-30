//! Server-owned JSON viewer-access v1. No client email or group string is an entitlement.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewerAudience {
    Public,
    Restricted,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum ViewerAccessChange {
    SetAudience {
        audience: ViewerAudience,
        confirm_metadata_rights: bool,
    },
    GrantActor {
        grant_id: String,
        actor_id: String,
        #[serde(deserialize_with = "nullable_expiry")]
        expires_at_us: Option<i64>,
    },
    GrantGroup {
        grant_id: String,
        group_id: String,
        #[serde(deserialize_with = "nullable_expiry")]
        expires_at_us: Option<i64>,
    },
    RevokeGrant {
        grant_id: String,
    },
    ShareMailbox {
        share_id: String,
        email: String,
    },
    RevokeShare {
        share_id: String,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewerAccessRequest {
    pub operation_id: String,
    pub expected_version: u64,
    pub change: ViewerAccessChange,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ViewerAccessReceipt {
    pub operation_id: String,
    pub work_id: String,
    pub version: u64,
    pub replayed: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ViewerPrincipal {
    Actor { actor_id: String },
    Group { group_id: String },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewerGrant {
    pub grant_id: String,
    pub principal: ViewerPrincipal,
    #[serde(deserialize_with = "nullable_expiry")]
    pub expires_at_us: Option<i64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PendingViewerShare {
    pub share_id: String,
    pub email: String,
    pub expires_at_us: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewerAccessState {
    pub work_id: String,
    pub version: u64,
    pub audience: ViewerAudience,
    pub grants: Vec<ViewerGrant>,
    pub pending: Vec<PendingViewerShare>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum ViewerGroupChange {
    Create {
        name: String,
    },
    AddMember {
        actor_id: String,
        #[serde(deserialize_with = "nullable_expiry")]
        expires_at_us: Option<i64>,
    },
    RemoveMember {
        actor_id: String,
    },
    Revoke,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewerGroupRequest {
    pub operation_id: String,
    pub expected_version: u64,
    pub change: ViewerGroupChange,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ViewerGroupReceipt {
    pub operation_id: String,
    pub group_id: String,
    pub version: u64,
    pub replayed: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewerGroupMember {
    pub actor_id: String,
    #[serde(deserialize_with = "nullable_expiry")]
    pub expires_at_us: Option<i64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewerGroupState {
    pub group_id: String,
    pub name: String,
    pub version: u64,
    pub revoked: bool,
    pub members: Vec<ViewerGroupMember>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewerGroups {
    pub items: Vec<ViewerGroupState>,
}

fn nullable_expiry<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<i64>, D::Error> {
    Option::<i64>::deserialize(deserializer)
}
