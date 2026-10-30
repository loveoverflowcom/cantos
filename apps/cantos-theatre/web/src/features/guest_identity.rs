//! One shared generated identity on Me and navigation; guests retain their original glyph.
use super::common::PersonOutlineGlyph;
use crate::contract::account_presentation::AccountPresentation;
use cantos_ui::{Avatar, AvatarInitials, AvatarSize, AvatarTone};
use leptos::prelude::*;

/// Display-only Server presentation; no actor identifier or authentication decision enters markup.
#[component]
pub(crate) fn ProfileAvatar(
    #[prop(optional)] navigation: bool,
    #[prop(optional, into)] identity: Signal<Option<AccountPresentation>>,
) -> impl IntoView {
    let size = if navigation {
        AvatarSize::Control
    } else {
        AvatarSize::Large
    };
    let guest_size = if navigation {
        AvatarSize::Small
    } else {
        AvatarSize::Medium
    };
    view! {
        <span class="theatre-avatar theatre-profile-avatar" data-profile="true" data-navigation-avatar=navigation.then_some("true") data-personalized=move || identity.get().is_some().then_some("true") aria-hidden="true">
            {move || match identity.get() {
                Some(person) => view! {
                    <Avatar display_name=person.alias stable_id=Some(person.avatar_seed) size
                        initials=AvatarInitials::Pair tone=AvatarTone::Lavender/>
                }.into_any(),
                None => view! {
                    <Avatar display_name="" size=guest_size accent=true/>
                    <span class="theatre-profile-glyph"><PersonOutlineGlyph/></span>
                }.into_any(),
            }}
        </span>
    }
}
