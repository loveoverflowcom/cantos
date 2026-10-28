//! Typed resources for live and mock listener surfaces; story fixtures retain their language.
use crate::contract::discovery::WorkKind;

#[cfg(feature = "preview")]
use crate::core::catalog::Genre;
use crate::{
    app::routes::Route,
    core::playback::{Failure, PauseCause, Status},
};

pub use crate::localization::Locale;

pub use crate::localization::generated::shell::{text, Key};

pub fn episode_count(locale: Locale, count: usize) -> String {
    let result = u64::try_from(count)
        .map_err(|_| cantos_localization::FormatError::OutOfRange)
        .and_then(|count| {
            crate::localization::generated::Message::TheatreShellEpisodeCount { arg_count: count }
                .render(locale)
        });
    crate::localization::present(result, locale)
}

pub fn spoken_time(locale: Locale, position: u32) -> String {
    use crate::localization::generated::Message;
    let result = (|| {
        let minutes = Message::TheatreShellMinuteCount {
            arg_count: u64::from(position / 60 % 60),
        }
        .render(locale)?;
        let seconds = Message::TheatreShellSecondCount {
            arg_count: u64::from(position % 60),
        }
        .render(locale)?;
        if position >= 3600 {
            let hours = Message::TheatreShellHourCount {
                arg_count: u64::from(position / 3600),
            }
            .render(locale)?;
            Message::TheatreShellSpokenTimeHours {
                arg_hours: &hours,
                arg_minutes: &minutes,
                arg_seconds: &seconds,
            }
            .render(locale)
        } else {
            Message::TheatreShellSpokenTime {
                arg_minutes: &minutes,
                arg_seconds: &seconds,
            }
            .render(locale)
        }
    })();
    crate::localization::present(result, locale)
}

pub fn position_description(locale: Locale, position: u32, duration: u32) -> String {
    message(
        locale,
        crate::localization::generated::Message::TheatreShellSpokenTimeOf {
            arg_elapsed: &(spoken_time(locale, position)),
            arg_total: &(spoken_time(locale, duration)),
        },
    )
}

pub fn page_title(locale: Locale, route: Route) -> String {
    let page = match route {
        Route::Work(_) | Route::PublicWork(_) => text(Key::AboutWork, locale),
        route => text(route_key(route), locale),
    };
    message(
        locale,
        crate::localization::generated::Message::TheatreShellPageTitle { arg_page: page },
    )
}

/// Speed in hundredths with the locale decimal mark and no trailing zeros, the same contract as
/// CMP `ShellCopy.speed`: 75 → `0,75×` / `0.75×`, 100 → `1×`, 150 → `1,5×` / `1.5×`.
pub fn speed_label(locale: Locale, speed: u16) -> String {
    let result = cantos_localization::Formatter::try_new(locale.lang()).and_then(|formatter| {
        let value = formatter.hundredths(speed);
        crate::localization::generated::Message::TheatreShellSpeedLabel { arg_value: &value }
            .render(locale)
    });
    crate::localization::present(result, locale)
}

/// Public catalog kinds have one app-owned mapping, independent of either feature.
pub fn work_kind_key(kind: WorkKind) -> Key {
    match kind {
        WorkKind::Novel => Key::PublicWorkKindNovel,
        WorkKind::Story => Key::PublicWorkKindStory,
        WorkKind::Podcast => Key::PublicWorkKindPodcast,
    }
}

#[cfg(feature = "preview")]
pub fn genre_key(genre: Genre) -> Key {
    match genre {
        Genre::All => Key::All,
        Genre::Fantasy => Key::Fantasy,
        Genre::Memory => Key::Memory,
        Genre::Dream => Key::Dream,
    }
}

pub fn route_key(route: Route) -> Key {
    match route {
        Route::Discover => Key::Discover,
        Route::Explore => Key::Explore,
        Route::Work(_) | Route::PublicWork(_) => Key::AboutWork,
        Route::Library => Key::Library,
        Route::History => Key::History,
        Route::Player => Key::Player,
        Route::Queue => Key::Queue,
        Route::Settings => Key::Settings,
        Route::Account => Key::Account,
        Route::Auth(_) => Key::SignIn,
        Route::Membership(_) => Key::MembershipHeading,
        Route::NotFound => Key::NotFound,
    }
}

#[cfg(feature = "preview")]
pub fn status_key(status: Status) -> Key {
    match status {
        Status::Idle => Key::NoPlayer,
        Status::Loading => Key::PlayerLoading,
        Status::Paused(PauseCause::User) => Key::Paused,
        Status::Paused(PauseCause::Interruption) => Key::Interrupted,
        Status::Paused(PauseCause::ColdRestore) => Key::ResumeOffered,
        Status::Playing => Key::Simulating,
        Status::Buffering => Key::Buffering,
        Status::Ended => Key::Ended,
        Status::Failed(failure) => failure_key(failure),
    }
}

/// The status of a real engine: nothing here is simulated.
pub fn listening_status_key(status: Status) -> Key {
    match status {
        Status::Idle => Key::ListenNothingSelected,
        Status::Loading => Key::ListenPreparing,
        Status::Paused(PauseCause::User) => Key::ListenPaused,
        Status::Paused(PauseCause::Interruption) => Key::ListenInterrupted,
        Status::Paused(PauseCause::ColdRestore) => Key::ListenResumeOffered,
        Status::Playing => Key::ListenPlaying,
        Status::Buffering => Key::ListenBuffering,
        Status::Ended => Key::ListenEnded,
        Status::Failed(failure) => failure_key(failure),
    }
}

pub fn failure_key(failure: Failure) -> Key {
    match failure {
        Failure::Timeout => Key::ErrorTimeout,
        Failure::Unauthenticated => Key::ErrorUnauthenticated,
        Failure::Forbidden => Key::ErrorForbidden,
        Failure::Configuration => Key::ErrorConfiguration,
        Failure::PlayRefused => Key::ListenFailedPlayRefused,
        Failure::Unavailable => Key::ListenFailedUnavailable,
        Failure::Network => Key::ListenFailedNetwork,
        Failure::Decode => Key::ListenFailedDecode,
        Failure::Protocol => Key::ListenFailedProtocol,
        Failure::IdentityChanged => Key::ListenFailedIdentity,
    }
}

pub const fn error_key(kind: crate::application::errors::ErrorKind) -> Key {
    match kind {
        crate::application::errors::ErrorKind::Network => Key::ErrorNetwork,
        crate::application::errors::ErrorKind::Timeout => Key::ErrorTimeout,
        crate::application::errors::ErrorKind::Unauthenticated => Key::ErrorUnauthenticated,
        crate::application::errors::ErrorKind::Forbidden => Key::ErrorForbidden,
        crate::application::errors::ErrorKind::ContentUnavailable => Key::ErrorContentUnavailable,
        crate::application::errors::ErrorKind::Protocol => Key::ErrorProtocol,
        crate::application::errors::ErrorKind::Configuration => Key::ErrorConfiguration,
        crate::application::errors::ErrorKind::Storage => Key::ErrorStorage,
        crate::application::errors::ErrorKind::ServiceUnavailable => Key::ErrorServiceUnavailable,
    }
}

pub fn message(locale: Locale, message: crate::localization::generated::Message<'_>) -> String {
    crate::localization::present(message.render(locale), locale)
}

pub fn clock_seconds(locale: Locale, seconds: u64) -> String {
    crate::localization::present(
        cantos_localization::Formatter::try_new(locale.lang())
            .map(|formatter| formatter.clock(seconds)),
        locale,
    )
}

pub fn public_unit_count(kind: WorkKind, locale: Locale, count: u64) -> String {
    use crate::localization::generated::Message;
    message(
        locale,
        match kind {
            WorkKind::Podcast => {
                Message::TheatreShellPublicWorkUnitCountEpisodes { arg_count: count }
            }
            WorkKind::Novel | WorkKind::Story => {
                Message::TheatreShellPublicWorkUnitCountChapters { arg_count: count }
            }
        },
    )
}

pub fn ordinal_label(locale: Locale, zero_based: usize) -> String {
    let number = u64::try_from(zero_based)
        .ok()
        .and_then(|value| value.checked_add(1))
        .ok_or(cantos_localization::FormatError::OutOfRange);
    crate::localization::present(
        number.and_then(|number| {
            cantos_localization::Formatter::try_new(locale.lang())
                .map(|formatter| formatter.integer(number, 2))
        }),
        locale,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn placeholders(value: &str) -> BTreeMap<&str, usize> {
        let mut names = BTreeMap::new();
        for name in value
            .split('{')
            .skip(1)
            .filter_map(|part| part.split_once('}').map(|pair| pair.0))
        {
            *names.entry(name).or_default() += 1;
        }
        names
    }

    #[test]
    fn every_resource_has_both_locales_and_matching_parameters() {
        for key in Key::ALL {
            let vi = text(*key, Locale::Vietnamese);
            let en = text(*key, Locale::English);
            assert!(!vi.trim().is_empty() && !en.trim().is_empty(), "{key:?}");
            assert_eq!(placeholders(vi), placeholders(en), "{key:?}");
        }
    }

    #[test]
    fn locale_tags_parse_exactly_and_name_themselves() {
        for (locale, lang, autonym) in [
            (Locale::Vietnamese, "vi-VN", "Tiếng Việt"),
            (Locale::English, "en", "English"),
        ] {
            assert_eq!(locale.lang(), lang);
            assert_eq!(locale.autonym(), autonym);
            assert_eq!(Locale::from_lang(lang), Some(locale));
        }
        for unsupported in ["", "fr", "not a locale", "ja"] {
            assert_eq!(Locale::from_lang(unsupported), None, "{unsupported}");
        }
        for (tag, expected) in [
            ("vi", Locale::Vietnamese),
            ("EN", Locale::English),
            ("en-US", Locale::English),
        ] {
            assert_eq!(Locale::from_lang(tag), Some(expected));
        }
        assert_eq!(Locale::default(), Locale::Vietnamese);
    }

    #[test]
    fn typed_values_are_opaque_and_rendered_as_escaped_text() {
        use crate::localization::generated::Message;
        use leptos::prelude::*;
        let title = "<b onclick=\"x()\">Sấm</b> {time}";
        let label = message(
            Locale::English,
            Message::TheatreShellBookmarkName {
                arg_title: title,
                arg_time: "12:05",
            },
        );
        assert_eq!(
            label,
            "Continue listening to <b onclick=\"x()\">Sấm</b> {time} at 12:05"
        );
        let html = view! { <button aria-label=label.clone()>{label.clone()}</button> }.to_html();
        assert!(!html.contains("<b "), "{html}");
        assert!(html.contains("&lt;b"), "{html}");
    }

    #[test]
    fn plural_categories_follow_cldr_for_each_locale() {
        for count in [0, 1, 2, 21, 101] {
            assert_eq!(
                cantos_localization::Formatter::try_new("vi-VN")
                    .unwrap()
                    .cardinal(&count.to_string())
                    .unwrap(),
                cantos_localization::PluralCategory::Other
            );
        }
        assert_eq!(
            cantos_localization::Formatter::try_new("en")
                .unwrap()
                .cardinal("0")
                .unwrap(),
            cantos_localization::PluralCategory::Other
        );
        assert_eq!(
            cantos_localization::Formatter::try_new("en")
                .unwrap()
                .cardinal("1")
                .unwrap(),
            cantos_localization::PluralCategory::One
        );
        assert_eq!(
            cantos_localization::Formatter::try_new("en")
                .unwrap()
                .cardinal("2")
                .unwrap(),
            cantos_localization::PluralCategory::Other
        );
        assert_eq!(
            cantos_localization::Formatter::try_new("en")
                .unwrap()
                .cardinal("21")
                .unwrap(),
            cantos_localization::PluralCategory::Other
        );
        for (count, vi, en) in [
            (0, "0 tập", "0 episodes"),
            (1, "1 tập", "1 episode"),
            (2, "2 tập", "2 episodes"),
        ] {
            assert_eq!(episode_count(Locale::Vietnamese, count), vi);
            assert_eq!(episode_count(Locale::English, count), en);
        }
    }

    /// Same vectors as CMP `DemoCopyTest`; a change here is a cross-renderer product change.
    #[test]
    fn speed_and_spoken_time_match_the_cmp_shell_copy_contract() {
        for (speed, vi, en) in [
            (75, "0,75×", "0.75×"),
            (100, "1×", "1×"),
            (125, "1,25×", "1.25×"),
            (150, "1,5×", "1.5×"),
            (200, "2×", "2×"),
        ] {
            assert_eq!(speed_label(Locale::Vietnamese, speed), vi);
            assert_eq!(speed_label(Locale::English, speed), en);
        }
        for (seconds, vi, en) in [
            (0, "0 phút 0 giây", "0 minutes 0 seconds"),
            (1, "0 phút 1 giây", "0 minutes 1 second"),
            (59, "0 phút 59 giây", "0 minutes 59 seconds"),
            (60, "1 phút 0 giây", "1 minute 0 seconds"),
            (61, "1 phút 1 giây", "1 minute 1 second"),
            (768, "12 phút 48 giây", "12 minutes 48 seconds"),
            (1722, "28 phút 42 giây", "28 minutes 42 seconds"),
        ] {
            assert_eq!(spoken_time(Locale::Vietnamese, seconds), vi);
            assert_eq!(spoken_time(Locale::English, seconds), en);
        }
    }

    #[test]
    fn public_kind_mapping_and_named_templates_have_both_locales() {
        for (kind, key, vi, en) in [
            (
                WorkKind::Novel,
                Key::PublicWorkKindNovel,
                "Tiểu thuyết",
                "Novel",
            ),
            (WorkKind::Story, Key::PublicWorkKindStory, "Truyện", "Story"),
            (
                WorkKind::Podcast,
                Key::PublicWorkKindPodcast,
                "Podcast",
                "Podcast",
            ),
        ] {
            assert_eq!(work_kind_key(kind), key);
            assert_eq!(text(key, Locale::Vietnamese), vi);
            assert_eq!(text(key, Locale::English), en);
        }
        for locale in Locale::supported() {
            let label = message(
                locale,
                crate::localization::generated::Message::TheatreShellCoverArtName {
                    arg_title: "Lời hẹn {description}",
                    arg_description: "Bìa",
                },
            );
            assert_eq!(label, "Lời hẹn {description}: Bìa");
        }
    }
}
