//! Pure preview discovery over the generated shared registrations.

use super::messages::tool;
use super::preview_catalog::{
    ComponentRegistration, DesignSpecimen, TokenRegistration, COMPONENTS,
};
use cantos_ui::IconName;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Family {
    pub key: &'static str,
    pub label: &'static str,
    pub anchor: &'static str,
    pub icon: IconName,
}

pub const FAMILIES: &[Family] = &[
    Family {
        key: "foundation",
        label: tool::FAMILY_FOUNDATION,
        anchor: "gallery-settings",
        icon: IconName::LightMode,
    },
    Family {
        key: "actions",
        label: tool::FAMILY_ACTIONS,
        anchor: "gallery-actions",
        icon: IconName::Add,
    },
    Family {
        key: "navigation",
        label: tool::FAMILY_NAVIGATION,
        anchor: "gallery-navigation",
        icon: IconName::MenuBook,
    },
    Family {
        key: "fields",
        label: tool::FAMILY_FIELDS,
        anchor: "gallery-fields",
        icon: IconName::Search,
    },
    Family {
        key: "feedback",
        label: tool::FAMILY_FEEDBACK,
        anchor: "gallery-feedback",
        icon: IconName::WarningAmber,
    },
    Family {
        key: "layout",
        label: tool::FAMILY_LAYOUT,
        anchor: "gallery-layout",
        icon: IconName::MenuBook,
    },
    Family {
        key: "icons",
        label: tool::FAMILY_ICONS,
        anchor: "gallery-icons",
        icon: IconName::Check,
    },
    Family {
        key: "dialog",
        label: tool::FAMILY_DIALOG,
        anchor: "gallery-dialog-samples",
        icon: IconName::Close,
    },
];

pub fn family_key(value: &str) -> &'static str {
    FAMILIES
        .iter()
        .find(|family| family.key == value)
        .map_or("all", |family| family.key)
}

pub fn sample_anchor(family: &str) -> Option<&'static str> {
    FAMILIES
        .iter()
        .find(|candidate| candidate.key == family)
        .map(|family| family.anchor)
}

fn words_match(query: &str, searchable: &str) -> bool {
    let searchable = searchable.to_lowercase();
    query
        .split_whitespace()
        .all(|word| searchable.contains(&word.to_lowercase()))
}

pub fn component_matches(component: &ComponentRegistration, family: &str, query: &str) -> bool {
    if family != "all" && component.family != family {
        return false;
    }
    let family_label = FAMILIES
        .iter()
        .find(|family| family.key == component.family)
        .map_or("", |family| family.label);
    words_match(
        query,
        &format!(
            "{} {} {} {} {} {} {}",
            component.id,
            component.name,
            component.family,
            family_label,
            component.states.join(" "),
            component.leptos.status,
            component.cmp.status
        ),
    )
}

pub fn family_matches(family: &str, selected: &str, query: &str) -> bool {
    COMPONENTS.iter().any(|component| {
        component.family == family && component_matches(component, selected, query)
    })
}

pub fn token_matches(token: &TokenRegistration, query: &str) -> bool {
    words_match(
        query,
        &format!(
            "{} {} {} {} {} {} {}",
            token.name,
            token.kind,
            token.light,
            token.dark,
            token.compact.unwrap_or_default(),
            token.comfortable.unwrap_or_default(),
            token.alias_of.unwrap_or_default()
        ),
    )
}

/// Design discovery is independent of public API registration and runtime admission.
pub fn design_matches(specimen: &DesignSpecimen, status: &str, scope: &str, query: &str) -> bool {
    (status == "all" || specimen.design_status == status)
        && (scope == "all" || specimen.scope == scope)
        && words_match(
            query,
            &format!(
                "{} {} {} {} {} {} {} {} {} {} {}",
                specimen.id,
                specimen.name,
                specimen.family,
                specimen.owner,
                specimen.consumer,
                specimen.description,
                specimen.states.join(" "),
                specimen.anatomy.join(" "),
                specimen.token_roles.join(" "),
                specimen.component_ids.join(" "),
                specimen.scope,
            ),
        )
}

pub fn specimen_registration(specimen: &DesignSpecimen) -> Option<&'static ComponentRegistration> {
    COMPONENTS
        .iter()
        .find(|component| component.id == specimen.id)
}
