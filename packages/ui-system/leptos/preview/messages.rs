//! Preview-owned copy. No Studio/Theatre catalog or persisted locale policy is imported.
//!
//! The developer tool shell is deliberately fixed English on both renderers. Recipe locale
//! selects the component specimen only. Vietnamese typography/content fixtures stay verbatim
//! and carry their own language tag; they are not translations of the shell.

use super::recipe::{Fixture, Locale, Recipe};

pub mod tool {
    pub const LANGUAGE_TAG: &str = "en";
    pub const GALLERY_TITLE: &str = "Component gallery";
    pub const GALLERY_INTRO: &str = "Try component states, long Vietnamese fixtures and keyboard input. Changes stay in this local preview.";
    pub const GALLERY_NAV: &str = "Gallery sections";
    pub const DISPLAY: &str = "Display";
    pub const EXISTING_API: &str = "Existing APIs";
    pub const SETTINGS: &str = "Display settings";
    pub const THEME: &str = "Theme";
    pub const LIGHT: &str = "Light";
    pub const DARK: &str = "Dark";
    pub const SYSTEM: &str = "System default";
    pub const COMFORTABLE: &str = "Comfortable density";
    pub const DENSITY_HELP: &str =
        "Turn off for compact density; touch targets retain the component contract.";
    pub const SHOW_ERRORS: &str = "Show sample errors";
    pub const ERRORS_HELP: &str =
        "Errors are associated with fields; this does not validate entered data.";
    pub const TEXT_SCALE: &str = "Text at 200%";
    pub const TEXT_SCALE_HELP: &str =
        "Scale text at the composition root; layout reflows and entered values are preserved.";
    pub const REDUCED_MOTION: &str = "Reduce sample motion";
    pub const MOTION_HELP: &str =
        "Suppress nonessential travel and repetition; turn off to follow browser policy.";
    pub const FONT_THEME: &str = "Vietnamese fonts and nested themes";
    pub const FONT_HELP: &str = "Vietnamese fixtures use bundled, licensed Noto Sans / Noto Serif 400 and 700. Check font readiness before capture; the gallery fetches no service artwork or audio.";
    pub const NAVIGATION: &str = "Native link navigation";
    pub const GO_FIELDS: &str = "Go to field samples";
    pub const NAV_HELP: &str = "CMP has disabled native tabs and in-place callbacks; HTML anchors still navigate when actions are blocked. NavTabs is link navigation. Native Tab, Enter, Ctrl/Cmd-click and copy-link behavior remain available. Settings and navigation remain visible during filtering.";
    pub const ACTIONS: &str = "Actions and filters";
    pub const ACTION_STATE: &str = "Button and chip state";
    pub const READY: &str = "Ready";
    pub const WAITING: &str = "Loading";
    pub const DISABLED: &str = "Disabled";
    pub const DISABLED_REASON: &str = "Disabled with reason";
    pub const LOADING_REASON: &str = "Loading with reason";
    pub const NATIVE_HELP: &str = "Use Tab, Enter and Space on native controls; observe hover, pressed and focus-visible with real input. Links remain navigable when actions are blocked.";
    pub const FIELDS: &str = "Controlled fields and choices";
    pub const FIELD_STATE: &str = "Sample field state";
    pub const EDITABLE: &str = "Editable";
    pub const READONLY: &str = "Read-only; text selection and copy remain available";
    pub const FIELD_HELP: &str = "Read-only applies to text, textarea and number fields. Select, checkbox, radio, switch and range use native enabled/disabled/loading states.";
    pub const FEEDBACK: &str = "Badges, status, loading, empty and error";
    pub const FEEDBACK_STATE: &str = "Sample feedback state";
    pub const EMPTY: &str = "Empty";
    pub const LOADING: &str = "Loading";
    pub const LOADING_ANNOUNCEMENT: &str = "Loading with polite announcement";
    pub const ERROR: &str = "Error";
    pub const SUCCESS: &str = "Success";
    pub const LAYOUT: &str = "Composed layout and reading region";
    pub const ICONS: &str = "Official outlined icons";
    pub const ICONS_HELP: &str = "Icons are decorative and do not supply an accessible name; samples include adjacent text. The offline set preserves attributed source geometry.";
    pub const AVATARS: &str = "Initial avatars";
    pub const AVATARS_HELP: &str = "Default single initials use a stable user color. Sample images are offline data URIs; failed images fall back to initials.";
    pub const DIALOG: &str = "Externally controlled native dialog";
    pub const DIALOG_HELP: &str = "Open the dialog, try Tab / Shift+Tab, Escape and its close action. Check focus returns to the trigger and entered text survives reopening.";
    pub const FOOTER: &str = "This gallery demonstrates Leptos components. Rendering, keyboard, focus, screen-reader and reduced-motion acceptance require separate observations.";
    pub const TOP: &str = "Back to gallery top";
    pub const AUTH_ACTIONS: &str = "Auth/account actions · Design v2";
    pub const AUTH_ACTION_HELP: &str = "Hold a real press to observe shape; use Tab to observe focus. These local intents do not sign in or create accounts.";
    pub const AUTH_ACTION_STATE: &str = "Auth action sample state";
    pub const AUTH_FIELDS: &str = "Auth tonal fields with persistent labels";
    pub const AUTH_FIELD_STATE: &str = "Auth field sample state";
    pub const AUTH_EDITABLE: &str = "Editable";
    pub const AUTH_READONLY: &str = "Read-only, selectable and copyable";
    pub const TOGGLE_ERROR: &str = "Toggle sample error";
    pub const AUTH_CANDIDATE: &str = "Floating (Fields v3) · Web-only. Use Tab to compare focus with the existing AuthTonal counterpart below. Qualification is recorded separately.";
    pub const AUTH_BASELINE: &str = "Existing AuthTonal · compatibility baseline:";
    pub const AUTH_REGULAR: &str = "Regular field with unchanged geometry";
    pub const BACK_HEADING: &str = "Contextual Back · host-owned destination";
    pub const BACK_STATE: &str = "Back sample state";
    pub const BACK_BASELINE: &str =
        "Unified Web candidate · unqualified. Existing IconTonal compatibility baseline:";
    pub const BACK_CONTEXT: &str = "Change sample return context";
    pub const WORKBENCH_BRAND: &str = "Cantos · Leptos · local diagnostics";
    pub const WORKBENCH_TITLE: &str = "Scenario workbench";
    pub const WORKBENCH_INTRO: &str = "Select a recipe, copy its link, drive real input, then capture and inspect. No network, identity or audio effects.";
    pub const RECIPE: &str = "Scenario recipe";
    pub const COMPONENT: &str = "Component / variant";
    pub const INITIAL_STATE: &str = "Initial state";
    pub const MODE: &str = "View mode";
    pub const FIXTURE: &str = "Fixture";
    pub const DENSITY: &str = "Density";
    pub const SPECIMEN_LANGUAGE: &str = "Specimen language";
    pub const SPECIMEN_TEXT: &str = "Specimen text scale (%)";
    pub const FRAME: &str = "Frame width (CSS px, not viewport)";
    pub const MOTION: &str = "Motion (force-full overrides OS: opt-in diagnostic)";
    pub const PLAYBACK: &str = "Review: replay resets, drive real input";
    pub const RESET: &str = "Reset to selected recipe";
    pub const REPLAY_NOTICE: &str = "Replay ready: press/release/cancel or use keyboard on the actual control. No synthetic pressed state.";
    pub const REPLAY: &str = "Replay input";
    pub const REFRESH: &str = "Refresh browser / font facts";
    pub const EXPORT: &str = "Export metadata";
    pub const LINK: &str = "Copy scenario link / reload recipe";
    pub const PROVENANCE: &str = "Recipe and capture provenance";
    pub const NATIVE_POLICY: &str = "Native activation policy: click/Enter/Space; browser-owned slop, axis and cancel. No custom drag threshold or velocity. Dialog: Escape/close, native focus containment/restore. Sheet and mini↔full player gestures have no shared consumer here and remain unsupported.";
    pub const FRAME_NAME: &str = "Specimen frame, horizontal scroll when wider than browser";
    pub const INTENTS_NAME: &str = "Real component intents";
    pub const INTENTS: &str = "Intent log";
    pub const MOTION_NAME: &str = "Motion observations";
    pub const MOTION_PROFILE: &str = "Trusted input frame profile";
    pub const CAPTURE_LABEL: &str = "Capture label (local diagnostic text)";
    pub const MOTION_DESCRIPTION: &str = "Computed styles at start/middle/end; interruptions and unavailable middle frames stay explicit. This is a bounded observation, not a video or pixel review.";
    pub const AUTH_RHYTHM: &str = "Floating (Fields v3) Web specimen. Real input is required for hover/focus/autofill/forced-colors; qualification is recorded separately. CTA logs an intent only.";
    pub const AUTH_RHYTHM_BASELINE: &str =
        "Existing AuthTonal presentation and 16-unit field→CTA gap (compatibility baseline):";
    pub const BACK_CANDIDATE: &str = "Unified Web candidate — unqualified. Hover, pressed and keyboard focus require real input. Every sample records a host intent only; no route/history effect.";
    pub const ICON_BASELINE: &str = "Existing IconTonal presentation (compatibility baseline):";
    pub const SELECT_DENSITIES: &str = "Regular theme: 48. Compact inline language: 40 painted / 48 target, 20 icon. Native compact baseline below; unsupported Popover uses the same native geometry. No Button/TextField/CMP resizing.";
    pub const MENU_CANDIDATE: &str = "Enhanced Web menu specimen. Native fallback on unsupported engines. Qualification is recorded separately; expanded/hover/focus require real input, never forced visual states.";
    pub const SAMPLE_DIALOG: &str = "Sample dialog";
    pub const CLOSE_SAMPLE: &str = "Close sample";
    pub const BROWSER_PENDING: &str = "Browser facts not captured; refresh after fonts are ready.";
    pub const EXPORTED: &str =
        "Metadata exported. Capture/open pixels and record input separately.";
    pub const CATALOG_TITLE: &str = "Registered components";
    pub const CATALOG_HELP: &str = "Search by API, family or state. Filtering only hides specimens; entered text, selections and event counts are retained.";
    pub const CATALOG_SEARCH: &str = "Find a component";
    pub const CATALOG_PLACEHOLDER: &str = "For example: TextField, loading, dialog";
    pub const CATALOG_SEARCH_HELP: &str =
        "Every word must match, case-insensitively. Search is limited to the local catalog.";
    pub const CATALOG_FAMILY: &str = "Component family";
    pub const CATALOG_ALL: &str = "All families";
    pub const CATALOG_RESET: &str = "Show all samples";
    pub const CATALOG_EMPTY: &str =
        "No components match. Clear the search or select all families to restore the samples.";
    pub const CATALOG_DETAILS: &str = "APIs, sample states and renderer status";
    pub const CATALOG_ID: &str = "ID / family";
    pub const CATALOG_STATES: &str = "Sample states";
    pub const TOKEN_TITLE: &str = "Named tokens from the shared source";
    pub const TOKEN_SEARCH: &str = "Find a token";
    pub const TOKEN_PLACEHOLDER: &str = "For example: color.primary, shape, 3rem";
    pub const TOKEN_EMPTY: &str =
        "No tokens match. Clear the search to see the complete shared source.";
    pub const TOKEN_VARIABLE: &str = "CSS variable";
    pub const TOKEN_COMPACT: &str = "Compact density";
    pub const TOKEN_FALLBACK: &str = "No override; use the theme value";
    pub const TOKEN_ORIGINAL: &str = "Original role";
    pub const FAMILY_NAME: &str = "Component families in the gallery";
    pub const DESIGN_TITLE: &str = "Find the right specimen before building a screen";
    pub const DESIGN_FLOW: &str = "Find → reuse or extend → register and exercise missing specimens → verify → admit → adopt in a product.";
    pub const DESIGN_HELP: &str = "Proposals name an owner, consumer, anatomy and intended states. Registration and runtime qualification remain separate; a design specimen creates no public API.";
    pub const DESIGN_SEARCH: &str = "Search design v3";
    pub const DESIGN_PLACEHOLDER: &str = "For example: mini-player, Studio, focus, color.primary";
    pub const DESIGN_STATUS: &str = "Design status";
    pub const DESIGN_ALL: &str = "All designs";
    pub const DESIGN_EXISTING: &str = "Existing APIs";
    pub const DESIGN_PROPOSED: &str = "Proposed; admission pending";
    pub const DESIGN_SCOPE: &str = "Ownership scope";
    pub const SCOPE_ALL: &str = "All scopes";
    pub const DESIGN_RESET: &str = "Clear design filters";
    pub const DESIGN_EMPTY: &str = "No specimens match. Clear the filters to restore them.";
    pub const DESIGN_DETAILS: &str = "Inspect anatomy, states and tokens";
    pub const DESIGN_OWNER: &str = "Owner / scope";
    pub const DESIGN_CONSUMER: &str = "Intended consumer";
    pub const DESIGN_MATRIX: &str = "Design state matrix";
    pub const DESIGN_REUSE: &str = "Reused APIs";
    pub const DESIGN_REASON: &str = "Rationale / admission";
    pub const DESIGN_PENDING: &str = "planned · no public API; unqualified";
    pub const DESIGN_BASELINE: &str = "Retained baseline states";
    pub const DESIGN_SAMPLE: &str = "View the existing API sample below";
    pub const DESIGN_LIMITS: &str = "Source and comparison limits";
    pub const DESIGN_SOURCE_HELP: &str = "Handoff PNGs are offline static exports. Browser comparison needs an HTML reference at the same viewport and state; images do not establish interaction or motion.";
    pub const DESIGN_MANIFEST: &str = "Design v3 manifest on Drive";
    pub const PROTOTYPE_TITLE: &str = "Try compositions for the next consumer";
    pub const PROTOTYPE_HELP: &str = "Proposed fixtures · planned · unqualified. They reuse existing primitives and export no new public API.";
    pub const PROTOTYPE_LIMITS: &str = "Scenario v3-compositions: long Vietnamese · position 0 · empty/loading/error/review · input retained across theme, density and text scale changes. Product admission remains pending.";
    pub const TOKEN_VALUE_ONLY: &str =
        "Values and units are in the table below; this does not animate or simulate a viewport.";
    pub const FAMILY_FOUNDATION: &str = "Foundation";
    pub const FAMILY_ACTIONS: &str = "Actions";
    pub const FAMILY_NAVIGATION: &str = "Navigation";
    pub const FAMILY_FIELDS: &str = "Fields";
    pub const FAMILY_FEEDBACK: &str = "Feedback";
    pub const FAMILY_LAYOUT: &str = "Layout";
    pub const FAMILY_ICONS: &str = "Icons";
    pub const FAMILY_DIALOG: &str = "Dialog";
    pub const TOKEN_CAPTION_COLOR: &str = "Current-theme color sample";
    pub const TOKEN_CAPTION_FONT_FAMILY: &str = "Font sample; fallback depends on installed fonts";
    pub const TOKEN_CAPTION_FONT_SIZE: &str = "Semantic font-size sample";
    pub const TOKEN_CAPTION_FONT_WEIGHT: &str = "Semantic font-weight sample";
    pub const TOKEN_CAPTION_LINE_HEIGHT: &str = "Two-line text-rhythm sample";
    pub const TOKEN_CAPTION_SPACING: &str = "Actual spacing between sample marks";
    pub const TOKEN_CAPTION_SHAPE: &str = "Semantic corners on a static specimen";
    pub const TOKEN_CAPTION_SIZE: &str = "Actual dimensions of a static specimen";
    pub const TOKEN_CAPTION_STROKE: &str = "Actual sample stroke width";
    pub const TOKEN_CAPTION_SHADOW: &str = "Semantic shadow on a static specimen";
    pub const TOKEN_CAPTION_OPACITY: &str = "Semantic opacity color layer";
    pub const TOKEN_CAPTION_VALUE_ONLY: &str =
        "Read-only parameter; no breakpoint or motion simulation";
    pub fn component_count(count: usize, total: usize) -> String {
        format!("{count} / {total} components match. Registration is renderer evidence, not runtime qualification.")
    }
    pub fn token_source(version: &str) -> String {
        format!("Token source v{version}. CSS values are read-only; inspect both themes and density overrides. No preview-specific palette is created.")
    }
    pub fn token_count(count: usize, total: usize) -> String {
        format!("{count} / {total} tokens match")
    }
    pub fn design_count(count: usize, total: usize, apis: usize) -> String {
        format!("{count} / {total} specimens match · {apis} public APIs · qualification needs separate evidence")
    }
    pub fn renderer_status(leptos: &str, cmp: &str) -> String {
        format!("{leptos} / {cmp} · qualification needs separate evidence")
    }
    pub const DIALOG_NOT_OPENED: &str = "Dialog has not been opened.";
    pub const DIALOG_REQUEST_OPEN: &str = "Sample dialog open requested.";
    pub const DIALOG_FINISHED: &str = "Closed with the sample content action.";
    pub fn received_intents(count: u32) -> String {
        format!("Received intents: {count}")
    }
    pub fn input_events(count: u32, value: &str) -> String {
        format!("Input events: {count} · Value: {value}")
    }
    pub fn back_events(count: u32) -> String {
        format!("Host received Back intents: {count}; no history, sign-in or sign-out call.")
    }
    pub fn controlled_value(value: &str) -> String {
        format!("Controlled value: {value}")
    }
    pub fn dialog_events(count: u32, event: &str) -> String {
        format!("Open requests: {count} · {event}")
    }
    pub fn dialog_closed(reason: impl std::fmt::Debug) -> String {
        format!("Close requested: {reason:?}.")
    }
    pub fn dialog_error(error: impl std::fmt::Debug) -> String {
        format!("Dialog operation unavailable: {error:?}.")
    }
    pub const FONT_PENDING: &str = "NOT_RUN: browser fonts have not been checked.";
    pub const FONT_CHECK: &str = "Check the four declared Noto fonts";
    pub const FONT_NO_DOCUMENT: &str = "UNSUPPORTED: browser document is unavailable.";
    pub const FONT_NO_SET: &str = "UNSUPPORTED: FontFaceSet could not be read.";
    pub const FONT_NO_OBSERVATIONS: &str = "UNSUPPORTED: font observations could not be read.";
    pub const FONT_LOAD_FAILED: &str = "FAIL: font loading failed";
    pub const FONT_LOADING: &str = "NOT_RUN: font is still loading";
    pub const FONT_UNDECLARED: &str = "UNSUPPORTED: face is not declared";
    pub const FONT_NEEDS_BROWSER: &str = "NOT_RUN: font readiness requires a real browser.";
    pub const LOGO_HEADING: &str = "Official logos · local vector assets";
    pub const LOGO_HELP: &str = "Lettering is vector paths. Backgrounds below belong to the specimens, not the logo. Monochrome variants retain transparent negative space.";
    pub const LOGO_SYMBOL: &str = "Symbol";
    pub const LOGO_LIGHT: &str = "light background";
    pub const LOGO_DARK: &str = "dark background";
    pub const LOGO_MONO: &str = "monochrome / transparent negative space";
    pub const LOGO_DECORATIVE: &str = "Cantos · decorative symbol beside its name";
    pub const LOGO_NAMED: &str = "Standalone symbol with an accessible name";
    pub const LOGO_ASPECT: &str = "A narrow frame retains the aspect ratio";
}

/// Fixed endonyms identify languages in either specimen locale; never translate them.
pub const VIETNAMESE_AUTONYM: &str = "Tiếng Việt";
pub const ENGLISH_AUTONYM: &str = "English";

/// Labels of actual recipe specimens, independent from the fixed-English tool shell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpecimenKey {
    Blocked,
    Email,
    EmailLong,
    EmailHelp,
    EmailError,
    Continue,
    Account,
    AccountLong,
    SignIn,
    System,
    Light,
    Dark,
    Unavailable,
    Theme,
    ThemePlaceholder,
    ThemeHelp,
    ThemeError,
    Language,
    OpenDialog,
    PreservedText,
    SaveDraft,
    SignInTitle,
}

impl SpecimenKey {
    pub const ALL: &'static [Self] = &[
        Self::Blocked,
        Self::Email,
        Self::EmailLong,
        Self::EmailHelp,
        Self::EmailError,
        Self::Continue,
        Self::Account,
        Self::AccountLong,
        Self::SignIn,
        Self::System,
        Self::Light,
        Self::Dark,
        Self::Unavailable,
        Self::Theme,
        Self::ThemePlaceholder,
        Self::ThemeHelp,
        Self::ThemeError,
        Self::Language,
        Self::OpenDialog,
        Self::PreservedText,
        Self::SaveDraft,
        Self::SignInTitle,
    ];
}

pub const fn specimen(locale: Locale, key: SpecimenKey) -> &'static str {
    match (locale, key) {
        (Locale::Vi, SpecimenKey::Blocked) => "Trạng thái mẫu bị khóa; đổi recipe để tiếp tục.",
        (Locale::En, SpecimenKey::Blocked) => {
            "This sample is blocked; change its recipe to continue."
        }
        (Locale::Vi, SpecimenKey::Email) => "Email của bạn",
        (Locale::En, SpecimenKey::Email) => "Your email",
        (Locale::Vi, SpecimenKey::EmailLong) => {
            "Email của Nguyễn Thị Ánh Nguyệt để tiếp tục câu chuyện đang nghe"
        }
        (Locale::En, SpecimenKey::EmailLong) => {
            "Nguyễn Thị Ánh Nguyệt’s email to continue the story you are listening to"
        }
        (Locale::Vi, SpecimenKey::EmailHelp) => {
            "Mẫu cục bộ giữ giá trị; không gửi email hoặc tạo tài khoản."
        }
        (Locale::En, SpecimenKey::EmailHelp) => {
            "Local sample preserves your value; no email or account is created."
        }
        (Locale::Vi, SpecimenKey::EmailError) => {
            "Kiểm tra địa chỉ email. Nội dung đã nhập được giữ nguyên."
        }
        (Locale::En, SpecimenKey::EmailError) => {
            "Check the email address. Your entered value is preserved."
        }
        (Locale::Vi, SpecimenKey::Continue) => "Tiếp tục — mẫu cục bộ",
        (Locale::En, SpecimenKey::Continue) => "Continue — local sample",
        (Locale::Vi, SpecimenKey::Account) => "Về Tài khoản",
        (Locale::En, SpecimenKey::Account) => "Back to Account",
        (Locale::Vi, SpecimenKey::AccountLong) => {
            "Về Tài khoản của Nguyễn Thị Ánh Nguyệt, giữ nguyên ý định đăng ký Membership"
        }
        (Locale::En, SpecimenKey::AccountLong) => {
            "Back to Nguyễn Thị Ánh Nguyệt’s account, keeping the Membership registration context"
        }
        (Locale::Vi, SpecimenKey::SignIn) => "Về Đăng nhập",
        (Locale::En, SpecimenKey::SignIn) => "Back to Sign in",
        (Locale::Vi, SpecimenKey::System) => "Theo hệ thống",
        (Locale::En, SpecimenKey::System) => "System default",
        (Locale::Vi, SpecimenKey::Light) => "Sáng",
        (Locale::En, SpecimenKey::Light) => "Light",
        (Locale::Vi, SpecimenKey::Dark) => "Tối",
        (Locale::En, SpecimenKey::Dark) => "Dark",
        (Locale::Vi, SpecimenKey::Unavailable) => "Không khả dụng",
        (Locale::En, SpecimenKey::Unavailable) => "Unavailable",
        (Locale::Vi, SpecimenKey::Theme) => "Giao diện",
        (Locale::En, SpecimenKey::Theme) => "Theme",
        (Locale::Vi, SpecimenKey::ThemePlaceholder) => "Chọn giao diện",
        (Locale::En, SpecimenKey::ThemePlaceholder) => "Choose a theme",
        (Locale::Vi, SpecimenKey::ThemeHelp) => {
            "Dùng bàn phím hoặc chạm để chọn. Escape giữ lựa chọn cũ."
        }
        (Locale::En, SpecimenKey::ThemeHelp) => {
            "Use the keyboard or touch to choose. Escape keeps the previous value."
        }
        (Locale::Vi, SpecimenKey::ThemeError) => "Lựa chọn không hợp lệ; chọn lại giao diện.",
        (Locale::En, SpecimenKey::ThemeError) => "Invalid choice; choose a theme again.",
        (Locale::Vi, SpecimenKey::Language) => "Ngôn ngữ",
        (Locale::En, SpecimenKey::Language) => "Language",
        (Locale::Vi, SpecimenKey::OpenDialog) => "Mở hộp thoại mẫu",
        (Locale::En, SpecimenKey::OpenDialog) => "Open sample dialog",
        (Locale::Vi, SpecimenKey::PreservedText) => "Văn bản được giữ",
        (Locale::En, SpecimenKey::PreservedText) => "Preserved text",
        (Locale::Vi, SpecimenKey::SaveDraft) => "Lưu bản nháp: {title}",
        (Locale::En, SpecimenKey::SaveDraft) => "Save draft: {title}",
        (Locale::Vi, SpecimenKey::SignInTitle) => "Đăng nhập Cantos: {title}",
        (Locale::En, SpecimenKey::SignInTitle) => "Sign in to Cantos: {title}",
    }
}

/// Expand placeholders from the original template once; inserted titles are opaque text.
pub fn format_specimen(locale: Locale, key: SpecimenKey, parameters: &[(&str, &str)]) -> String {
    let template = specimen(locale, key);
    let mut output = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        output.push_str(&rest[..start]);
        let candidate = &rest[start + 1..];
        let Some(end) = candidate.find('}') else {
            output.push_str(&rest[start..]);
            return output;
        };
        let name = &candidate[..end];
        if let Some((_, value)) = parameters.iter().find(|(key, _)| *key == name) {
            output.push_str(value);
        } else {
            output.push_str(&rest[start..start + end + 2]);
        }
        rest = &candidate[end + 1..];
    }
    output.push_str(rest);
    output
}

pub const fn fixture_label(recipe: Recipe) -> &'static str {
    match (recipe.locale, recipe.fixture) {
        (Locale::Vi, Fixture::Standard) => "Ngày mai, mình có diễn tiếp không?",
        (Locale::En, Fixture::Standard) => "Shall we perform again tomorrow?",
        (Locale::Vi, Fixture::LongVietnamese) => "Nguyễn Thị Ánh Nguyệt — Ánh đèn cuối sân khấu và lời hẹn dưới chân Thanh Vân Sơn. Ngày mai, mình có diễn tiếp không?",
        (Locale::En, Fixture::LongVietnamese) => "Nguyễn Thị Ánh Nguyệt — The last stage light and a promise beneath Thanh Vân Sơn. Shall we perform again tomorrow?",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn parameters(template: &str) -> BTreeMap<&str, usize> {
        let mut found = BTreeMap::new();
        for part in template.split('{').skip(1) {
            if let Some((name, _)) = part.split_once('}') {
                *found.entry(name).or_insert(0) += 1;
            }
        }
        found
    }

    #[test]
    fn all_typed_specimen_resources_have_both_locales_and_matching_named_parameters() {
        for &key in SpecimenKey::ALL {
            let vi = specimen(Locale::Vi, key);
            let en = specimen(Locale::En, key);
            assert!(!vi.trim().is_empty(), "missing Vietnamese for {key:?}");
            assert!(!en.trim().is_empty(), "missing English for {key:?}");
            assert_eq!(
                parameters(vi),
                parameters(en),
                "parameter mismatch for {key:?}"
            );
        }
    }

    #[test]
    fn specimen_titles_are_inserted_once_and_keep_literal_parameter_syntax() {
        assert_eq!(
            format_specimen(
                Locale::Vi,
                SpecimenKey::SaveDraft,
                &[("title", "Lời hẹn {time}"), ("time", "12:05")]
            ),
            "Lưu bản nháp: Lời hẹn {time}"
        );
        assert_eq!(
            format_specimen(
                Locale::En,
                SpecimenKey::SignInTitle,
                &[("title", "Lời hẹn {time}"), ("time", "12:05")]
            ),
            "Sign in to Cantos: Lời hẹn {time}"
        );
    }

    #[test]
    fn long_vietnamese_typography_fixture_is_preserved_as_repository_content() {
        let vi = Recipe {
            fixture: Fixture::LongVietnamese,
            locale: Locale::Vi,
            ..Recipe::default()
        };
        assert_eq!(fixture_label(vi), "Nguyễn Thị Ánh Nguyệt — Ánh đèn cuối sân khấu và lời hẹn dưới chân Thanh Vân Sơn. Ngày mai, mình có diễn tiếp không?");
        assert_eq!(tool::LANGUAGE_TAG, "en");
    }
}
