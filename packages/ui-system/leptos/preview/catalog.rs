//! Pure preview discovery over the generated shared registrations.

use super::preview_catalog::{ComponentRegistration, TokenRegistration, COMPONENTS};
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
        label: "Nền tảng",
        anchor: "gallery-settings",
        icon: IconName::LightMode,
    },
    Family {
        key: "actions",
        label: "Hành động",
        anchor: "gallery-actions",
        icon: IconName::Add,
    },
    Family {
        key: "navigation",
        label: "Điều hướng",
        anchor: "gallery-navigation",
        icon: IconName::MenuBook,
    },
    Family {
        key: "fields",
        label: "Nhập liệu",
        anchor: "gallery-fields",
        icon: IconName::Search,
    },
    Family {
        key: "feedback",
        label: "Trạng thái",
        anchor: "gallery-feedback",
        icon: IconName::WarningAmber,
    },
    Family {
        key: "layout",
        label: "Bố cục",
        anchor: "gallery-layout",
        icon: IconName::MenuBook,
    },
    Family {
        key: "icons",
        label: "Biểu tượng",
        anchor: "gallery-icons",
        icon: IconName::Check,
    },
    Family {
        key: "dialog",
        label: "Hộp thoại",
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
