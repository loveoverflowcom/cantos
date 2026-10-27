//! Generated parameters are required and typed. The successful control uses the same API.
//! ```
//! use cantos_theatre::localization::{Locale, generated::Message};
//! assert!(Message::TheatreShellEpisodeCount { arg_count: 2 }.render(Locale::English).is_ok());
//! ```
//! ```compile_fail
//! use cantos_theatre::localization::generated::Message;
//! let _ = Message::TheatreShellEpisodeCount { arg_count: "2" };
//! ```
//! ```compile_fail
//! use cantos_theatre::localization::generated::Message;
//! let _ = Message::TheatreShellEpisodeCount {};
//! ```
//! App-owned resource projection and final presentation of typed formatter refusals.
#[rustfmt::skip]
pub mod generated;
pub use generated::{Locale, LocaleError, Resolution};

pub fn present(result: Result<String, cantos_localization::FormatError>, locale: Locale) -> String {
    match result {
        Ok(value) => value,
        Err(_) => generated::locale::text(generated::locale::Key::FormatUnavailable, locale).into(),
    }
}

pub use cantos_localization::{PreferenceSelection, PreferenceWarning};

pub fn preference_warning(warning: Option<PreferenceWarning>, locale: Locale) -> &'static str {
    use generated::locale::{text, Key};
    match warning {
        None => "",
        Some(PreferenceWarning::Unsupported) => text(Key::UnsupportedPreference, locale),
        Some(PreferenceWarning::StorageUnavailable) => text(Key::PreferenceUnavailable, locale),
    }
}

#[cfg(target_arch = "wasm32")]
const PREFERENCE_KEY: &str = "cantos.theatre.ui-locale.v1";

#[cfg(target_arch = "wasm32")]
pub fn load_preference() -> PreferenceSelection<Locale> {
    use cantos_localization::{select_preference, StoredPreference};
    let stored = web_sys::window()
        .ok_or(())
        .and_then(|window| window.local_storage().map_err(|_| ()))
        .and_then(|storage| storage.ok_or(()))
        .and_then(|storage| storage.get_item(PREFERENCE_KEY).map_err(|_| ()));
    let state = match &stored {
        Ok(Some(tag)) => StoredPreference::Saved(tag),
        Ok(None) => StoredPreference::Missing,
        Err(()) => StoredPreference::Unavailable,
    };
    select_preference(state, Locale::default(), Locale::from_lang)
}

#[cfg(target_arch = "wasm32")]
pub fn save_preference(locale: Locale) -> Result<(), PreferenceWarning> {
    web_sys::window()
        .ok_or(PreferenceWarning::StorageUnavailable)
        .and_then(|window| {
            window
                .local_storage()
                .map_err(|_| PreferenceWarning::StorageUnavailable)
        })
        .and_then(|storage| storage.ok_or(PreferenceWarning::StorageUnavailable))
        .and_then(|storage| {
            storage
                .set_item(PREFERENCE_KEY, locale.lang())
                .map_err(|_| PreferenceWarning::StorageUnavailable)
        })
}

/// The one projection of the supported UI locales for every language selector.
pub fn locale_options() -> Vec<cantos_ui::SelectOption> {
    Locale::supported()
        .map(|option| cantos_ui::SelectOption {
            lang: Some(option.lang().to_owned()),
            ..cantos_ui::SelectOption::new(option.lang(), option.autonym())
        })
        .collect()
}

pub fn theme_labels(locale: Locale) -> cantos_ui::ThemeCycleLabels {
    use generated::theme::{text, Key};
    cantos_ui::ThemeCycleLabels {
        dark_mode: text(Key::DarkMode, locale),
        dark_next: text(Key::DarkNext, locale),
        light_mode: text(Key::LightMode, locale),
        light_next: text(Key::LightNext, locale),
        system_mode: text(Key::SystemMode, locale),
        system_next: text(Key::SystemNext, locale),
    }
}

// Native HTML tests have no browser storage. Production persistence is WASM-only.
#[cfg(not(target_arch = "wasm32"))]
pub fn load_preference() -> PreferenceSelection<Locale> {
    cantos_localization::select_preference(
        cantos_localization::StoredPreference::Missing,
        Locale::default(),
        Locale::from_lang,
    )
}
#[cfg(not(target_arch = "wasm32"))]
pub fn save_preference(_: Locale) -> Result<(), PreferenceWarning> {
    Err(PreferenceWarning::StorageUnavailable)
}
