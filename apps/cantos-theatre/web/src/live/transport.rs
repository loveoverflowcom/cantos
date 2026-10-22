//! The credential policy of every Theatre request class, as pure facts with host guards.
//!
//! * Protected listener routes (Library, progress, operations) carry the Identity v1 cookie and
//!   the session fence, and only through `cantos_web_auth::browser::Lease::apply`.
//! * Session routes (login, session, renew, logout) belong to the shared session owner.
//! * Signup start/complete are anonymous: `credentials: omit`, no fence, never a cookie.
//! * Public catalog, publication and media requests stay anonymous (`credentials: omit`); a
//!   cookie never rides to media or any URL outside the API's Theatre surface.
use cantos_contracts::routes::THEATRE_BASE;

fn route(path: &str) -> &str {
    path.split('?').next().unwrap_or(path)
}

/// Routes whose answer depends on the signed-in listener.
pub(crate) fn is_protected(path: &str) -> bool {
    let route = route(path);
    ["library", "progress", "operations"]
        .into_iter()
        .any(|group| route.starts_with(&format!("{THEATRE_BASE}/{group}/")))
        || route == format!("{THEATRE_BASE}/library")
}

pub(crate) fn is_anonymous_identity(path: &str) -> bool {
    matches!(
        route(path),
        "/api/v1/theatre/identity/signup" | "/api/v1/theatre/identity/signup/complete"
    )
}

pub(crate) fn is_session_identity(path: &str) -> bool {
    matches!(
        route(path),
        "/api/v1/theatre/identity/login"
            | "/api/v1/theatre/identity/session"
            | "/api/v1/theatre/identity/session/renew"
    )
}

/// Anonymous catalog reads and playback authorization.
pub(crate) fn is_public(path: &str) -> bool {
    let route = route(path);
    ["works", "publications", "media"].into_iter().any(|group| {
        route == format!("{THEATRE_BASE}/{group}")
            || route.starts_with(&format!("{THEATRE_BASE}/{group}/"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::library::Section;
    use crate::core::public_ids::PublicWorkId;

    #[test]
    fn every_route_class_is_disjoint_so_a_cookie_policy_cannot_leak_across_classes() {
        let uuid = uuid::Uuid::from_u128(7).to_string();
        let work = PublicWorkId::parse(&uuid).unwrap();
        let protected = [
            format!("{THEATRE_BASE}/library/saved?presentation=1&limit=20"),
            super::super::library::saved_path(&uuid),
            super::super::library::progress_path(&uuid),
            super::super::library::operation_path(&uuid),
            super::super::library::HISTORY_START_PATH.to_owned(),
            super::super::library::HISTORY_CLEAR_PATH.to_owned(),
        ];
        let public = [
            super::super::discovery::works_path(&Default::default(), None),
            super::super::public_work::work_path(work),
            super::super::public_work::units_path(work, None),
            super::super::public_media::manifest_path(uuid::Uuid::from_u128(8)),
            super::super::public_media::authorization_path(uuid::Uuid::from_u128(8)),
            format!("{THEATRE_BASE}/media/pg1_secret"),
        ];
        let anonymous = [
            super::super::signup::SIGNUP_PATH.to_owned(),
            super::super::signup::COMPLETE_PATH.to_owned(),
        ];
        let session = [
            "/api/v1/theatre/identity/login".to_owned(),
            "/api/v1/theatre/identity/session".to_owned(),
            "/api/v1/theatre/identity/session/renew".to_owned(),
        ];
        for (name, group) in [
            ("protected", &protected[..]),
            ("public", &public[..]),
            ("anonymous", &anonymous[..]),
            ("session", &session[..]),
        ] {
            for path in group {
                let classes = [
                    is_protected(path),
                    is_public(path),
                    is_anonymous_identity(path),
                    is_session_identity(path),
                ];
                let expected = match name {
                    "protected" => 0,
                    "public" => 1,
                    "anonymous" => 2,
                    _ => 3,
                };
                for (index, class) in classes.into_iter().enumerate() {
                    assert_eq!(class, index == expected, "{name}: {path}");
                }
            }
        }
        leptos::prelude::Owner::new().with(|| {
            use crate::application::account_view::AccountViewState;
            use crate::features::auth_preview::{signed_in, PreviewActor};
            use leptos::prelude::*;
            let state = RwSignal::new(AccountViewState::default());
            signed_in(state, PreviewActor::A).unwrap();
            for section in Section::ALL {
                let request = state.write().begin_library(section, true).unwrap();
                let path = super::super::library::request_path(&request);
                assert!(is_protected(&path), "{path}");
            }
        });
    }

    #[test]
    fn near_miss_and_foreign_paths_are_never_classified_for_a_cookie() {
        for path in [
            "/api/v1/theatre",
            "/api/v1/theatre/libraryx/saved",
            "/api/v1/theatre/identity",
            "/api/v1/theatre/identity/signup/other",
            "/api/v1/studio/library/saved",
            "https://api.example/api/v1/theatre/library/saved",
            "/api/v1/theatre/../studio/library/saved",
        ] {
            assert!(!is_protected(path), "{path}");
            assert!(!is_anonymous_identity(path), "{path}");
            assert!(!is_session_identity(path), "{path}");
        }
    }

    /// Source-level regression guard: a Bearer header or an ambient-cookie omission can never
    /// return to the user-session transport files, and the anonymous/public adapters can never
    /// start sending cookies. Tokens are assembled so this file does not match itself.
    #[test]
    fn user_session_sources_have_no_bearer_and_anonymous_sources_have_no_cookie() {
        let bearer = ["\"Author", "ization\""].concat();
        let header_call = [".header()", ""].concat();
        let omit = ["Credentials::", "Omit"].concat();
        let include = ["Credentials::", "Include"].concat();
        let session_sources = [
            ("account.rs", include_str!("account.rs")),
            ("library.rs", include_str!("library.rs")),
            ("runtime.rs", include_str!("runtime.rs")),
            ("failure.rs", include_str!("failure.rs")),
        ];
        for (name, source) in session_sources {
            assert!(!source.contains(&bearer), "{name} must not send a Bearer");
            assert!(!source.contains(&header_call), "{name}");
            assert!(!source.contains(&omit), "{name} is a user-session adapter");
            assert!(
                !source.contains("ca1_") && !source.contains("cr1_"),
                "{name}"
            );
        }
        for (name, source) in [
            ("signup.rs", include_str!("signup.rs")),
            ("discovery.rs", include_str!("discovery.rs")),
            ("public_work.rs", include_str!("public_work.rs")),
            ("public_media.rs", include_str!("public_media.rs")),
        ] {
            assert!(!source.contains(&include), "{name} is anonymous");
            assert!(!source.contains(&bearer), "{name}");
        }
        for (name, source) in [
            ("signup.rs", include_str!("signup.rs")),
            ("public_work.rs", include_str!("public_work.rs")),
            ("public_media.rs", include_str!("public_media.rs")),
        ] {
            // Wasm fetch adapters declare omission explicitly.
            assert!(
                source.contains(&omit),
                "{name} must declare credentials omit"
            );
        }
        // The one cookie policy owner applies credentials and the fence: both the paged read and
        // the point/mutation sender go through it.
        assert!(include_str!("library.rs").matches(".apply(").count() >= 2);
    }
}
