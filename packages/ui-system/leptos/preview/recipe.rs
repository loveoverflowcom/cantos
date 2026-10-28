//! Bounded, versioned local preview recipes. No service, browser or storage dependency.

use cantos_ui::{Density, Theme};
use std::fmt;

pub const RECIPE_VERSION: &str = "1";
/// Bounds apply before allocation or percent decoding of caller-controlled query input.
pub const MAX_RECIPE_QUERY_BYTES: usize = 1024;
pub const MAX_RECIPE_FIELDS: usize = 12;
pub const MAX_RECIPE_KEY_BYTES: usize = 16;
pub const MAX_RECIPE_VALUE_BYTES: usize = 32;

/// These are specimen widths in CSS pixels, never browser viewport measurements.
pub const REFLOW_FRAME_CSS_PX: u16 = 320;
pub const PHONE_FRAME_CSS_PX: u16 = 390;
pub const TABLET_FRAME_CSS_PX: u16 = 840;
pub const DESKTOP_FRAME_CSS_PX: u16 = 1440;
pub const NORMAL_TEXT_PERCENT: u16 = 100;
pub const DOUBLE_TEXT_PERCENT: u16 = 200;

macro_rules! recipe_enum {
    ($name:ident { $($variant:ident => $value:literal),+ $(,)? }) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum $name { $($variant),+ }

        impl $name {
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];

            pub const fn as_str(self) -> &'static str {
                match self { $(Self::$variant => $value),+ }
            }

            pub fn from_attribute(value: &str) -> Option<Self> {
                match value { $($value => Some(Self::$variant),)+ _ => None }
            }
        }
    };
}

recipe_enum!(ComponentId {
    Button => "button",
    ListenHero => "listen-hero",
    AuthPrimary => "auth-primary",
    FilterChip => "filter-chip",
    Switch => "switch",
    Dialog => "dialog",
    Select => "select",
    BackButton => "back-button",
    AuthTonal => "auth-tonal",
    EdgeProgress => "edge-progress",
});

recipe_enum!(ScenarioState {
    Ready => "ready",
    Resolving => "resolving",
    Buffering => "buffering",
    Playing => "playing",
    Paused => "paused",
    Failed => "failed",
    Unavailable => "unavailable",
    Enabled => "enabled",
    Disabled => "disabled",
    Loading => "loading",
    Unselected => "unselected",
    Selected => "selected",
    Off => "off",
    On => "on",
    Closed => "closed",
    Open => "open",
    Error => "error",
    MissingValue => "missing-value",
    UnsupportedOption => "unsupported-option",
    EmptyOptions => "empty-options",
    ReadOnly => "readonly",
    Idle => "idle",
    Pending => "pending",
    Success => "success",
    Cancel => "cancel",
    Hidden => "hidden",
    Background => "background",
    Disposed => "disposed",
    ListPending => "list-pending",
});

impl ComponentId {
    pub const fn states(self) -> &'static [ScenarioState] {
        use ScenarioState::*;
        match self {
            Self::ListenHero => &[
                Ready,
                Resolving,
                Buffering,
                Playing,
                Paused,
                Failed,
                Unavailable,
            ],
            Self::Button | Self::AuthPrimary | Self::BackButton => &[Enabled, Disabled, Loading],
            Self::FilterChip => &[Unselected, Selected, Disabled, Loading],
            Self::Switch => &[Off, On, Disabled, Loading],
            Self::Dialog => &[Closed, Open],
            Self::AuthTonal => &[Enabled, Error, Disabled, Loading, ReadOnly],
            Self::EdgeProgress => &[
                Idle,
                Pending,
                Success,
                Error,
                Cancel,
                Hidden,
                Background,
                Disposed,
                ListPending,
            ],
            Self::Select => &[
                Enabled,
                Disabled,
                Loading,
                Error,
                MissingValue,
                UnsupportedOption,
                EmptyOptions,
            ],
        }
    }

    pub const fn default_state(self) -> ScenarioState {
        match self {
            Self::ListenHero => ScenarioState::Ready,
            Self::Button | Self::AuthPrimary | Self::BackButton => ScenarioState::Enabled,
            Self::FilterChip => ScenarioState::Unselected,
            Self::Switch => ScenarioState::Off,
            Self::Dialog => ScenarioState::Closed,
            Self::Select => ScenarioState::Enabled,
            Self::AuthTonal => ScenarioState::Enabled,
            Self::EdgeProgress => ScenarioState::Idle,
        }
    }

    /// Auth primary is an existing Button variant, not a new public component API.
    pub const fn catalog_id(self) -> &'static str {
        match self {
            Self::AuthPrimary | Self::ListenHero => "button",
            Self::AuthTonal => "text-field",
            _ => self.as_str(),
        }
    }
}

recipe_enum!(Fixture { Standard => "standard", LongVietnamese => "long-vietnamese" });
recipe_enum!(Locale { Vi => "vi", En => "en" });
recipe_enum!(TextScale { Normal => "100", Double => "200" });
recipe_enum!(FrameWidth {
    Reflow => "320",
    Phone => "390",
    Tablet => "840",
    Desktop => "1440",
});
recipe_enum!(Motion { System => "system", Reduced => "reduced", ForceFull => "force-full" });
recipe_enum!(PreviewMode { Catalog => "catalog", Isolated => "isolated", Matrix => "matrix" });
recipe_enum!(Playback { Live => "live", Settled => "settled", Replay => "replay" });

impl Locale {
    pub const fn language_tag(self) -> &'static str {
        match self {
            Self::Vi => "vi-VN",
            Self::En => "en",
        }
    }
}

impl TextScale {
    pub const fn percent(self) -> u16 {
        match self {
            Self::Normal => NORMAL_TEXT_PERCENT,
            Self::Double => DOUBLE_TEXT_PERCENT,
        }
    }
}

impl FrameWidth {
    pub const fn css_px(self) -> u16 {
        match self {
            Self::Reflow => REFLOW_FRAME_CSS_PX,
            Self::Phone => PHONE_FRAME_CSS_PX,
            Self::Tablet => TABLET_FRAME_CSS_PX,
            Self::Desktop => DESKTOP_FRAME_CSS_PX,
        }
    }
}

recipe_enum!(RecipeField {
    Version => "v",
    Component => "component",
    State => "state",
    Fixture => "fixture",
    Theme => "theme",
    Density => "density",
    Locale => "locale",
    TextScale => "text",
    FrameWidth => "frame",
    Motion => "motion",
    Mode => "mode",
    Playback => "playback",
});

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RecipeError {
    QueryTooLong,
    TooManyFields,
    MalformedField,
    KeyTooLong,
    ValueTooLong,
    InvalidEncoding,
    UnknownField,
    DuplicateField(RecipeField),
    MissingVersion,
    UnsupportedVersion,
    InvalidValue(RecipeField),
    StateMismatch {
        component: ComponentId,
        state: ScenarioState,
    },
}

impl fmt::Display for RecipeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::QueryTooLong => {
                write!(formatter, "Recipe exceeds {MAX_RECIPE_QUERY_BYTES} bytes.")
            }
            Self::TooManyFields => write!(formatter, "Recipe exceeds {MAX_RECIPE_FIELDS} fields."),
            Self::MalformedField => {
                formatter.write_str("Recipe fields must have one key and one value.")
            }
            Self::KeyTooLong => formatter.write_str("A recipe key exceeds its byte limit."),
            Self::ValueTooLong => formatter.write_str("A recipe value exceeds its byte limit."),
            Self::InvalidEncoding => formatter.write_str("Recipe contains invalid URL encoding."),
            Self::UnknownField => formatter.write_str("Recipe contains an unknown field."),
            Self::DuplicateField(field) => {
                write!(formatter, "Recipe repeats the {} field.", field.as_str())
            }
            Self::MissingVersion => {
                formatter.write_str("Recipe version is missing; version 1 is required.")
            }
            Self::UnsupportedVersion => {
                formatter.write_str("Recipe version is unsupported; version 1 is required.")
            }
            Self::InvalidValue(field) => write!(
                formatter,
                "Recipe has an unsupported {} value.",
                field.as_str()
            ),
            Self::StateMismatch { component, state } => write!(
                formatter,
                "State {} is unavailable for {}.",
                state.as_str(),
                component.as_str()
            ),
        }
    }
}

/// Host-owned values; validate after composing a component/state edit before adopting it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Recipe {
    pub component: ComponentId,
    pub state: ScenarioState,
    pub fixture: Fixture,
    pub theme: Theme,
    pub density: Density,
    pub locale: Locale,
    pub text_scale: TextScale,
    pub frame: FrameWidth,
    pub motion: Motion,
    pub mode: PreviewMode,
    pub playback: Playback,
}

impl Default for Recipe {
    fn default() -> Self {
        Self {
            component: ComponentId::Button,
            state: ScenarioState::Enabled,
            fixture: Fixture::Standard,
            theme: Theme::Light,
            density: Density::Compact,
            locale: Locale::Vi,
            text_scale: TextScale::Normal,
            frame: FrameWidth::Phone,
            motion: Motion::System,
            mode: PreviewMode::Catalog,
            playback: Playback::Live,
        }
    }
}

impl Recipe {
    pub fn validate(self) -> Result<Self, RecipeError> {
        if !self.component.states().contains(&self.state) {
            return Err(RecipeError::StateMismatch {
                component: self.component,
                state: self.state,
            });
        }
        Ok(self)
    }

    /// Changing component chooses its actual initial state, not a forced CSS pseudo-state.
    pub fn with_component(self, component: ComponentId) -> Self {
        Self {
            component,
            state: component.default_state(),
            ..self
        }
    }

    pub fn with_state(self, state: ScenarioState) -> Result<Self, RecipeError> {
        Self { state, ..self }.validate()
    }

    pub fn scenario_id(self) -> String {
        format!(
            "{}/{}/{}",
            self.component.as_str(),
            self.state.as_str(),
            self.fixture.as_str()
        )
    }

    /// Canonical ordering makes copied links and evidence records stable.
    pub fn encode_query(self) -> String {
        format!(
            "?v={RECIPE_VERSION}&component={}&state={}&fixture={}&theme={}&density={}&locale={}&text={}&frame={}&motion={}&mode={}&playback={}",
            self.component.as_str(), self.state.as_str(), self.fixture.as_str(),
            self.theme.as_str(), self.density.as_str(), self.locale.as_str(),
            self.text_scale.as_str(), self.frame.as_str(), self.motion.as_str(),
            self.mode.as_str(), self.playback.as_str(),
        )
    }

    /// Rejection is atomic: callers choose and explain fallback rather than adopt a partial recipe.
    pub fn decode_query(query: &str) -> Result<Self, RecipeError> {
        if query.len() > MAX_RECIPE_QUERY_BYTES {
            return Err(RecipeError::QueryTooLong);
        }
        let query = query.strip_prefix('?').unwrap_or(query);
        if query.is_empty() {
            return Ok(Self::default());
        }
        let mut fields = Vec::new();
        for pair in query.split('&') {
            if fields.len() == MAX_RECIPE_FIELDS {
                return Err(RecipeError::TooManyFields);
            }
            let (key, value) = pair.split_once('=').ok_or(RecipeError::MalformedField)?;
            if key.is_empty() || value.is_empty() || value.contains('=') {
                return Err(RecipeError::MalformedField);
            }
            let key = decode_token(key, MAX_RECIPE_KEY_BYTES, RecipeError::KeyTooLong)?;
            let field = RecipeField::from_attribute(&key).ok_or(RecipeError::UnknownField)?;
            if fields.iter().any(|(seen, _)| *seen == field) {
                return Err(RecipeError::DuplicateField(field));
            }
            let value = decode_token(value, MAX_RECIPE_VALUE_BYTES, RecipeError::ValueTooLong)?;
            fields.push((field, value));
        }
        let value = |field| {
            fields
                .iter()
                .find(|(candidate, _)| *candidate == field)
                .map(|(_, value)| value.as_str())
        };
        match value(RecipeField::Version) {
            None => return Err(RecipeError::MissingVersion),
            Some(RECIPE_VERSION) => {}
            Some(_) => return Err(RecipeError::UnsupportedVersion),
        }
        let mut recipe = Self::default();
        if let Some(component) = value(RecipeField::Component) {
            recipe = recipe.with_component(parse_value(
                component,
                RecipeField::Component,
                ComponentId::from_attribute,
            )?);
        }
        for (field, value) in fields {
            let invalid = || RecipeError::InvalidValue(field);
            match field {
                RecipeField::Version | RecipeField::Component => {}
                RecipeField::State => {
                    recipe.state = ScenarioState::from_attribute(&value).ok_or_else(invalid)?
                }
                RecipeField::Fixture => {
                    recipe.fixture = Fixture::from_attribute(&value).ok_or_else(invalid)?
                }
                RecipeField::Theme => {
                    recipe.theme = Theme::from_attribute(&value).ok_or_else(invalid)?
                }
                RecipeField::Density => {
                    recipe.density = match value.as_str() {
                        "compact" => Density::Compact,
                        "comfortable" => Density::Comfortable,
                        _ => return Err(invalid()),
                    }
                }
                RecipeField::Locale => {
                    recipe.locale = Locale::from_attribute(&value).ok_or_else(invalid)?
                }
                RecipeField::TextScale => {
                    recipe.text_scale = TextScale::from_attribute(&value).ok_or_else(invalid)?
                }
                RecipeField::FrameWidth => {
                    recipe.frame = FrameWidth::from_attribute(&value).ok_or_else(invalid)?
                }
                RecipeField::Motion => {
                    recipe.motion = Motion::from_attribute(&value).ok_or_else(invalid)?
                }
                RecipeField::Mode => {
                    recipe.mode = PreviewMode::from_attribute(&value).ok_or_else(invalid)?
                }
                RecipeField::Playback => {
                    recipe.playback = Playback::from_attribute(&value).ok_or_else(invalid)?
                }
            }
        }
        recipe.validate()
    }
}

fn parse_value<T>(
    value: &str,
    field: RecipeField,
    parse: impl FnOnce(&str) -> Option<T>,
) -> Result<T, RecipeError> {
    parse(value).ok_or(RecipeError::InvalidValue(field))
}

/// Only URL-safe ASCII tokens are meaningful here; recipes never contain user text or URLs.
fn decode_token(value: &str, limit: usize, too_long: RecipeError) -> Result<String, RecipeError> {
    if value.len() > limit {
        return Err(too_long);
    }
    let mut decoded = String::with_capacity(value.len());
    let mut bytes = value.bytes();
    while let Some(byte) = bytes.next() {
        let byte = if byte == b'%' {
            let high = bytes
                .next()
                .and_then(hex_digit)
                .ok_or(RecipeError::InvalidEncoding)?;
            let low = bytes
                .next()
                .and_then(hex_digit)
                .ok_or(RecipeError::InvalidEncoding)?;
            high * 16 + low
        } else {
            byte
        };
        if !byte.is_ascii_alphanumeric() && byte != b'-' {
            return Err(RecipeError::InvalidEncoding);
        }
        decoded.push(char::from(byte));
    }
    Ok(decoded)
}

fn hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_query_selects_the_documented_initial_recipe() {
        assert_eq!(Recipe::decode_query(""), Ok(Recipe::default()));
        assert_eq!(Recipe::decode_query("?"), Ok(Recipe::default()));
        assert_eq!(Recipe::decode_query("?v=1"), Ok(Recipe::default()));
        assert_eq!(Recipe::default().frame.css_px(), 390);
        assert_eq!(Recipe::default().text_scale.percent(), 100);
    }

    #[test]
    fn copied_recipe_has_canonical_keys_and_restores_every_selected_fact() {
        let recipe = Recipe {
            component: ComponentId::AuthPrimary,
            state: ScenarioState::Loading,
            fixture: Fixture::LongVietnamese,
            theme: Theme::Dark,
            density: Density::Comfortable,
            locale: Locale::En,
            text_scale: TextScale::Double,
            frame: FrameWidth::Reflow,
            motion: Motion::Reduced,
            mode: PreviewMode::Isolated,
            playback: Playback::Settled,
        };
        let expected = "?v=1&component=auth-primary&state=loading&fixture=long-vietnamese&theme=dark&density=comfortable&locale=en&text=200&frame=320&motion=reduced&mode=isolated&playback=settled";
        assert_eq!(recipe.encode_query(), expected);
        assert_eq!(Recipe::decode_query(expected), Ok(recipe));
        assert_eq!(recipe.scenario_id(), "auth-primary/loading/long-vietnamese");
        assert_eq!(recipe.component.catalog_id(), "button");
    }

    #[test]
    fn disabled_current_select_recipe_is_not_accepted_by_other_controls() {
        let recipe = Recipe::decode_query("?v=1&component=select&state=unsupported-option")
            .expect("the registered disabled-current state needs a real host fixture");
        assert_eq!(recipe.component, ComponentId::Select);
        assert_eq!(recipe.state.as_str(), "unsupported-option");
        assert_eq!(Recipe::decode_query(&recipe.encode_query()), Ok(recipe));
        for &component in ComponentId::ALL {
            if component != ComponentId::Select {
                let query = format!(
                    "?v=1&component={}&state=unsupported-option",
                    component.as_str()
                );
                assert!(matches!(
                    Recipe::decode_query(&query),
                    Err(RecipeError::StateMismatch { .. })
                ));
            }
        }
    }

    #[test]
    fn all_supported_component_states_roundtrip_without_forced_css_pseudo_states() {
        for &component in ComponentId::ALL {
            for &state in component.states() {
                let recipe = Recipe {
                    component,
                    state,
                    ..Recipe::default()
                };
                assert_eq!(Recipe::decode_query(&recipe.encode_query()), Ok(recipe));
            }
        }
        assert_eq!(
            Recipe::decode_query("?v=1&component=button&state=pressed"),
            Err(RecipeError::InvalidValue(RecipeField::State))
        );
        assert_eq!(
            Recipe::decode_query("?v=1&component=dialog&state=loading"),
            Err(RecipeError::StateMismatch {
                component: ComponentId::Dialog,
                state: ScenarioState::Loading
            })
        );
    }

    #[test]
    fn every_supported_preference_value_roundtrips() {
        let initial = Recipe::default();
        let roundtrips = |recipe: Recipe| {
            assert_eq!(Recipe::decode_query(&recipe.encode_query()), Ok(recipe));
        };
        for theme in [Theme::System, Theme::Light, Theme::Dark] {
            roundtrips(Recipe { theme, ..initial });
        }
        for density in [Density::Compact, Density::Comfortable] {
            roundtrips(Recipe { density, ..initial });
        }
        for &fixture in Fixture::ALL {
            roundtrips(Recipe { fixture, ..initial });
        }
        for &locale in Locale::ALL {
            roundtrips(Recipe { locale, ..initial });
        }
        for &text_scale in TextScale::ALL {
            roundtrips(Recipe {
                text_scale,
                ..initial
            });
        }
        for &frame in FrameWidth::ALL {
            roundtrips(Recipe { frame, ..initial });
        }
        for &motion in Motion::ALL {
            roundtrips(Recipe { motion, ..initial });
        }
        for &mode in PreviewMode::ALL {
            roundtrips(Recipe { mode, ..initial });
        }
        for &playback in Playback::ALL {
            roundtrips(Recipe {
                playback,
                ..initial
            });
        }
    }

    #[test]
    fn unknown_versions_values_keys_and_duplicate_keys_are_atomic_refusals() {
        for (query, expected) in [
            ("?theme=dark", RecipeError::MissingVersion),
            ("?v=2&theme=dark", RecipeError::UnsupportedVersion),
            ("?v=01", RecipeError::UnsupportedVersion),
            (
                "?v=1&mode=unknown",
                RecipeError::InvalidValue(RecipeField::Mode),
            ),
            (
                "?v=1&component=sheet",
                RecipeError::InvalidValue(RecipeField::Component),
            ),
            (
                "?v=1&frame=391",
                RecipeError::InvalidValue(RecipeField::FrameWidth),
            ),
            (
                "?v=1&text=150",
                RecipeError::InvalidValue(RecipeField::TextScale),
            ),
            (
                "?v=1&locale=fr",
                RecipeError::InvalidValue(RecipeField::Locale),
            ),
            ("?v=1&theme=dark&unknown=value", RecipeError::UnknownField),
            (
                "?v=1&theme=light&theme=dark",
                RecipeError::DuplicateField(RecipeField::Theme),
            ),
            (
                "?v=1&v=1",
                RecipeError::DuplicateField(RecipeField::Version),
            ),
            (
                "?v=1&%74heme=dark&theme=light",
                RecipeError::DuplicateField(RecipeField::Theme),
            ),
        ] {
            assert_eq!(Recipe::decode_query(query), Err(expected), "{query}");
        }
    }

    #[test]
    fn malformed_or_unsafe_tokens_never_become_fixture_text_or_urls() {
        for query in ["?v", "?v=", "?=1", "?v=1&", "?v=1&theme=dark=light"] {
            assert_eq!(
                Recipe::decode_query(query),
                Err(RecipeError::MalformedField),
                "{query}"
            );
        }
        for query in [
            "?v=1&theme=%",
            "?v=1&theme=%2",
            "?v=1&theme=%GG",
            "?v=1&theme=%00",
            "?v=1&theme=%C3%A9",
            "?v=1&theme=dark+light",
            "?v=1&theme=dark#anchor",
            "?v=1&fixture=https://example.test",
        ] {
            assert_eq!(
                Recipe::decode_query(query),
                Err(RecipeError::InvalidEncoding),
                "{query}"
            );
        }
        assert_eq!(
            Recipe::decode_query("?%76=1&component=auth%2Dprimary&state=enabled"),
            Ok(Recipe::default().with_component(ComponentId::AuthPrimary))
        );
    }

    #[test]
    fn parser_rejects_oversized_input_before_decoding_or_allocation() {
        assert_eq!(
            Recipe::decode_query(&"x".repeat(MAX_RECIPE_QUERY_BYTES + 1)),
            Err(RecipeError::QueryTooLong)
        );
        assert_eq!(
            Recipe::decode_query(&format!("?{}=1", "k".repeat(MAX_RECIPE_KEY_BYTES + 1))),
            Err(RecipeError::KeyTooLong)
        );
        assert_eq!(
            Recipe::decode_query(&format!("?v={}", "1".repeat(MAX_RECIPE_VALUE_BYTES + 1))),
            Err(RecipeError::ValueTooLong)
        );
        let all_keys = RecipeField::ALL
            .iter()
            .map(|field| format!("{}=1", field.as_str()))
            .collect::<Vec<_>>()
            .join("&");
        assert_eq!(
            Recipe::decode_query(&format!("?{all_keys}&v=1")),
            Err(RecipeError::TooManyFields)
        );
    }

    #[test]
    fn selecting_another_component_keeps_environment_and_chooses_its_valid_initial_state() {
        let recipe = Recipe {
            theme: Theme::Dark,
            fixture: Fixture::LongVietnamese,
            ..Recipe::default()
        };
        let dialog = recipe.with_component(ComponentId::Dialog);
        assert_eq!(dialog.state, ScenarioState::Closed);
        assert_eq!(dialog.theme, Theme::Dark);
        assert_eq!(dialog.fixture, Fixture::LongVietnamese);
        assert_eq!(
            dialog
                .with_state(ScenarioState::Open)
                .map(|recipe| recipe.state),
            Ok(ScenarioState::Open)
        );
        assert_eq!(
            dialog.with_state(ScenarioState::Enabled),
            Err(RecipeError::StateMismatch {
                component: ComponentId::Dialog,
                state: ScenarioState::Enabled
            })
        );
    }
}
