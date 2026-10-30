//! Decorative account fallback until authorized profile facts are available.
use super::common::PersonOutlineGlyph;
use cantos_ui::{Avatar, AvatarSize};
use leptos::prelude::*;

/// Shared decorative fallback until the account projection supplies authorized profile facts.
#[component]
pub(crate) fn ProfileAvatar(#[prop(optional)] navigation: bool) -> impl IntoView {
    let size = if navigation {
        AvatarSize::Small
    } else {
        AvatarSize::Medium
    };
    view! {
        <span class="theatre-avatar theatre-profile-avatar" data-profile="true" data-navigation-avatar=navigation.then_some("true") aria-hidden="true">
            <Avatar display_name="" size accent=true/>
            <span class="theatre-profile-glyph"><PersonOutlineGlyph/></span>
        </span>
    }
}
