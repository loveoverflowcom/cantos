//! Theatre expression (token v0.7.0): the opt-in scope, the trailing field action, the docked
//! search kind, the control-size avatar and the CSS that scopes all Theatre geometry. DOM
//! string-render and stylesheet assertions only: no layout, focus or pixels are observed here.

use cantos_ui::{
    tokens, AccessibleName, Avatar, AvatarSize, ButtonState, DisabledReason, Expression,
    IconButton, IconName, TextField, TextFieldKind, TextFieldVariant, Theme, UiRoot,
    FOUNDATION_CSS, TOKEN_CSS,
};
use leptos::prelude::*;

fn name(text: &str) -> AccessibleName {
    AccessibleName::new(text).unwrap()
}

#[test]
fn search_body_shares_the_shell_and_marks_closed_content_inert() {
    Owner::new().with(|| {
        let open = RwSignal::new(false);
        let render = || view! {
            <TextField id="dock-query" label="Tìm truyện" kind=TextFieldKind::Search
                variant=TextFieldVariant::Floating value="  Quán Trà  " on_input=Callback::new(|_: String| {})
                dock=cantos_ui::SearchDock { id: "dock-results".into(), expanded: open.into(),
                    content: (|| view! { <button>"Kết quả thật do caller cung cấp"</button> }).into() }/>
        }.to_html();
        let closed = render();
        assert!(closed.contains("aria-expanded=\"false\""), "{closed}");
        assert!(closed.contains("aria-controls=\"dock-results\""), "{closed}");
        assert!(closed.contains("inert"), "{closed}");
        assert!(closed.contains("value=\"  Quán Trà  \""), "{closed}");
        open.set(true);
        let opened = render();
        assert!(opened.contains("aria-expanded=\"true\""), "{opened}");
        assert!(!opened.contains(" inert"), "{opened}");
        assert_eq!(opened.matches("cui-field-shell").count(), 1);
        assert_eq!(opened.matches("<input").count(), 1);
    });
}

fn eye() -> impl IntoView {
    view! {
        <IconButton id="eye" name=name("Hiện mật khẩu") icon=IconName::Visibility/>
    }
}

#[test]
fn expressions_in_rust_are_exactly_the_generated_canonical_inventory() {
    assert_eq!(Expression::Standard.attribute(), None);
    let named: Vec<&str> = [Expression::Standard, Expression::Theatre]
        .into_iter()
        .filter_map(Expression::attribute)
        .collect();
    assert_eq!(
        named,
        tokens::EXPRESSIONS,
        "a new canonical expression needs a Rust variant"
    );
}

#[test]
fn only_a_chosen_expression_adds_a_scope_attribute() {
    let standard = view! { <UiRoot theme=Theme::Light>"x"</UiRoot> }.to_html();
    assert!(!standard.contains("data-expression"), "{standard}");
    let theatre =
        view! { <UiRoot theme=Theme::Dark expression=Expression::Theatre>"x"</UiRoot> }.to_html();
    assert!(theatre.contains("data-expression=\"theatre\""), "{theatre}");
    assert!(theatre.contains("data-theme=\"dark\""), "{theatre}");
}

fn password_field(variant: TextFieldVariant) -> String {
    view! {
        <TextField id="password" variant=variant kind=TextFieldKind::Password label="Mật khẩu"
            value="" autocomplete="current-password" on_input=Callback::new(|_: String| {})
            trailing=eye/>
    }
    .to_html()
}

#[test]
fn a_floating_field_hosts_one_native_trailing_action_inside_its_shell_after_the_label() {
    let html = password_field(TextFieldVariant::Floating);
    assert!(html.contains("data-trailing=\"true\""), "{html}");
    assert_eq!(
        html.matches("class=\"cui-field-trailing\"").count(),
        1,
        "{html}"
    );
    let shell = html.find("class=\"cui-field-shell\"").unwrap();
    let input = html.find("<input id=\"password\"").unwrap();
    let label = html.find("for=\"password\"").unwrap();
    let trailing = html.find("cui-field-trailing").unwrap();
    let button = html.find("<button id=\"eye\"").unwrap();
    let shell_end = html.rfind("id=\"password-help\"").unwrap();
    assert!(
        shell < input && input < label && label < trailing && trailing < button,
        "{html}"
    );
    assert!(
        button < shell_end,
        "the action stays inside the shell, before the messages: {html}"
    );
    // A native, named control; never a clickable container.
    assert!(html.contains("type=\"button\""), "{html}");
    assert!(html.contains("title=\"Hiện mật khẩu\""), "{html}");
    assert!(
        html.contains("<span class=\"cui-sr-only\">Hiện mật khẩu</span>"),
        "{html}"
    );
    assert!(!html.contains("role=\"button\""), "{html}");
}

#[test]
fn the_trailing_slot_never_changes_the_native_input_contract() {
    let with = password_field(TextFieldVariant::Floating);
    let without = view! {
        <TextField id="password" variant=TextFieldVariant::Floating kind=TextFieldKind::Password
            label="Mật khẩu" value="" autocomplete="current-password" on_input=Callback::new(|_: String| {})/>
    }
    .to_html();
    let tag = |html: &str| {
        let start = html.find("<input id=\"password\"").unwrap();
        html[start..html[start..].find('>').unwrap() + start + 1].to_owned()
    };
    assert_eq!(
        tag(&with),
        tag(&without),
        "type, autocomplete, placeholder and ARIA are unchanged"
    );
    assert!(!without.contains("data-trailing"), "{without}");
    assert!(!without.contains("cui-field-trailing"), "{without}");
}

#[test]
fn only_the_shell_variant_hosts_a_trailing_action() {
    for variant in [
        TextFieldVariant::Regular,
        TextFieldVariant::AuthTonal,
        TextFieldVariant::AuthTonalSpacious,
    ] {
        assert!(!variant.supports_trailing());
        let html = password_field(variant);
        assert!(!html.contains("cui-field-trailing"), "{variant:?}: {html}");
        assert!(!html.contains("<button"), "{variant:?}: {html}");
    }
    assert!(TextFieldVariant::Floating.supports_trailing());
}

#[test]
fn the_field_kind_is_a_styling_hook_for_the_docked_search_and_changes_no_semantics() {
    for (kind, expected) in [
        (TextFieldKind::Search, "search"),
        (TextFieldKind::Password, "password"),
        (TextFieldKind::Email, "email"),
    ] {
        let html = view! {
            <TextField id="query" variant=TextFieldVariant::Floating kind=kind label="Tìm trong danh mục"
                value="" on_input=Callback::new(|_: String| {})/>
        }
        .to_html();
        assert!(
            html.contains(&format!("data-kind=\"{expected}\"")),
            "{html}"
        );
        assert!(html.contains(&format!("type=\"{expected}\"")), "{html}");
    }
}

#[test]
fn a_docked_submit_is_a_native_submit_named_by_the_host_and_never_fires_while_blocked() {
    let search = view! {
        <IconButton id="go" name=name("Tìm trong danh mục") icon=IconName::Search submit=true/>
    }
    .to_html();
    assert!(search.contains("type=\"submit\""), "{search}");
    assert!(
        search.contains("<span class=\"cui-sr-only\">Tìm trong danh mục</span>"),
        "{search}"
    );
    // Default is the old behavior: a plain button, so no existing form changes meaning.
    let plain = view! { <IconButton id="go" name=name("Đóng") icon=IconName::Close/> }.to_html();
    assert!(plain.contains("type=\"button\""), "{plain}");
    let blocked = view! {
        <IconButton id="go" name=name("Tìm") icon=IconName::Search submit=true
            state=ButtonState::Loading(DisabledReason::new("Đang tìm").unwrap())/>
    }
    .to_html();
    assert!(blocked.contains("disabled"), "{blocked}");
    assert!(blocked.contains("aria-busy=\"true\""), "{blocked}");
}

#[test]
fn the_control_avatar_is_a_distinct_size_and_the_accent_is_pinned_without_changing_initials() {
    let html = view! {
        <Avatar display_name="Cantos" size=AvatarSize::Control accent=true/>
    }
    .to_html();
    assert!(html.contains("data-size=\"control\""), "{html}");
    assert!(html.contains("data-tone=\"accent\""), "{html}");
    assert!(html.contains(">C</span>"), "{html}");
    let identity = view! { <Avatar display_name="Cantos" size=AvatarSize::Control/> }.to_html();
    assert!(!identity.contains("data-tone=\"accent\""), "{identity}");
    assert_eq!(AvatarSize::Control.as_str(), "control");
}

fn theatre_section() -> &'static str {
    let start = FOUNDATION_CSS
        .find("/* Theatre expression.")
        .expect("the Theatre expression section marker");
    &FOUNDATION_CSS[start..]
}

#[test]
fn every_theatre_rule_is_scoped_to_the_expression_so_standard_consumers_keep_their_geometry() {
    let mut rules = 0;
    for rule in theatre_section().split('}') {
        let Some((selector, body)) = rule.rsplit_once('{') else {
            continue;
        };
        let selector = selector.trim();
        if selector.is_empty() || selector.starts_with("/*") && !selector.contains("\n:where") {
            continue;
        }
        if selector.starts_with("@media") {
            continue;
        }
        rules += 1;
        assert!(
            selector.contains("[data-expression=\"theatre\"]"),
            "unscoped Theatre rule: {selector} {{{body}}}"
        );
    }
    assert!(
        rules >= 9,
        "expected the Theatre section's rules, found {rules}"
    );
    // The standard geometry and shapes are declared once, above the section.
    assert!(FOUNDATION_CSS.contains("--cantos-web-field-block-size: 3.625rem;"));
    assert!(
        !FOUNDATION_CSS[..FOUNDATION_CSS.find("/* Theatre expression.").unwrap()]
            .contains("data-expression")
    );
}

#[test]
fn theatre_geometry_matches_the_design_minimums_without_fixing_heights() {
    let css = theatre_section();
    for declaration in [
        "--cantos-web-field-block-size: 4.25rem;",
        "--cantos-web-field-radius: 1rem;",
        "--cantos-web-field-block-size: var(--cantos-size-search-header);",
        "--cantos-web-field-radius: var(--cantos-shape-search-rest);",
        "min-inline-size: var(--cantos-size-search-action);",
        "min-block-size: var(--cantos-size-search-action);",
    ] {
        assert!(css.contains(declaration), "missing {declaration}");
    }
    assert!(
        !css.contains(" height:") && !css.contains("\n  height:"),
        "no fixed height"
    );
    for rule in css.split('}') {
        let Some((selector, body)) = rule.rsplit_once('{') else {
            continue;
        };
        if !selector.contains(".cui-search-dock") {
            assert!(
                !body.contains("max-block-size") && !body.contains("overflow: hidden"),
                "only the dock body may clamp/clip, never the input header: {selector}"
            );
        }
    }
}

#[test]
fn a_chip_reads_by_shape_and_tone_with_an_interruptible_transition_that_reduced_motion_removes() {
    let css = theatre_section();
    assert!(css.contains(".cui-filter-chip[aria-pressed=\"true\"] {\n  border-radius: var(--cantos-web-chip-selected-radius);"));
    assert!(css.contains("border-radius var(--cantos-motion-shape-duration)"));
    assert!(
        css.contains(".cui-filter-chip .cui-chip-check {\n  display: none;"),
        "an unselected chip is centered on its label"
    );
    assert!(
        css.contains(
            ".cui-filter-chip:where([aria-pressed=\"true\"], [aria-busy=\"true\"]) .cui-chip-check {\n  display: inline-flex;"
        ),
        "selection or pending opens the slot that holds the check or the arc"
    );
    let reduced = css
        .rfind("@media (prefers-reduced-motion: reduce)")
        .unwrap();
    assert!(css[reduced..]
        .contains("[data-expression=\"theatre\"] .cui-filter-chip { transition: none; }"));
}

#[test]
fn a_theatre_tonal_action_is_pale_lilac_and_the_field_error_is_plain_text() {
    let css = theatre_section();
    assert!(css.contains(".cui-button:where([data-variant=\"tonal\"], [data-variant=\"auth-secondary\"]):not(:disabled) {\n  background-color: var(--cantos-color-surface-container-high);\n  color: var(--cantos-color-on-surface);"));
    assert!(css.contains(".cui-icon-button[data-variant=\"tonal\"] {\n  border-radius: var(--cantos-web-shape-reading);"));
    assert!(css.contains("> .cui-field-error {\n  padding: 0 var(--cantos-web-field-inset);\n  background: transparent;"));
    assert!(
        css.contains(".cui-field-shell {\n  background: var(--cantos-color-surface-container);")
    );
}

#[test]
fn the_field_perimeter_follows_its_control_and_the_trailing_action_keeps_its_own_ring() {
    assert!(FOUNDATION_CSS.contains(".cui-field-shell:has(> .cui-field-control:focus-visible)"));
    assert!(
        !FOUNDATION_CSS.contains(".cui-field-shell:focus-within"),
        "focus-within would stack the shell perimeter under a trailing action's ring"
    );
    assert!(FOUNDATION_CSS.contains("grid-template-columns: minmax(0, 1fr) auto;"));
    assert!(FOUNDATION_CSS.contains(".cui-field-trailing .cui-button:focus-visible"));
}

#[test]
fn the_canonical_remap_is_declared_after_every_theme_block_in_the_token_css() {
    let remap = TOKEN_CSS.find("[data-expression=\"theatre\"],").unwrap();
    assert!(remap > TOKEN_CSS.rfind("[data-theme=\"dark\"] {").unwrap());
    assert!(
        TOKEN_CSS[remap..].contains("--cantos-color-primary: var(--cantos-color-theatre-primary);")
    );
    assert!(TOKEN_CSS[remap..]
        .contains("--cantos-type-headline-weight: var(--cantos-type-theatre-heading-weight);"));
}

#[test]
fn the_docked_search_has_no_floating_label_and_strokes_stay_inside_its_own_bounds() {
    let css = theatre_section();
    // The label is the accessible name; it is visible only as the empty-state prompt.
    let hidden = css
        .find("[data-kind=\"search\"] .cui-field-shell > .cui-field-label {")
        .expect("hidden label rule");
    let prompt = css
        .find(".cui-field-control:placeholder-shown ~ .cui-field-label {")
        .expect("empty prompt rule");
    assert!(
        hidden < prompt,
        "the empty prompt overrides the hidden label"
    );
    let hidden_rule = &css[hidden..prompt];
    assert!(
        hidden_rule.contains("clip-path: inset(50%);"),
        "{hidden_rule}"
    );
    assert!(
        !hidden_rule.contains("display: none"),
        "the name stays in the tree"
    );
    // Idle 0, hover 1 inset, focus 2 inset: never a positive outline-offset or a halo.
    let hover = css
        .find(".cui-field-shell:hover:not([data-invalid=\"true\"])")
        .expect("hover rule");
    let hover_rule = &css[hover..hover + 420];
    assert!(
        hover_rule
            .contains("outline: var(--cantos-web-stroke-rest) solid var(--cantos-color-outline);"),
        "{hover_rule}"
    );
    assert!(
        hover_rule.contains("outline-offset: calc(-1 * var(--cantos-web-stroke-rest));"),
        "{hover_rule}"
    );
    assert!(!css.contains("box-shadow"), "no halo");
}

#[test]
fn the_docked_search_motion_is_a_web_seed_that_reduced_motion_zeroes_and_native_never_reads() {
    let css = FOUNDATION_CSS;
    for declaration in [
        "--cantos-web-search-open-duration: 280ms;",
        "--cantos-web-search-open-easing: cubic-bezier(0.2, 0, 0, 1);",
        "--cantos-web-search-close-duration: 200ms;",
        "--cantos-web-search-close-easing: cubic-bezier(0.4, 0, 1, 1);",
        "--cantos-web-search-effects-duration: var(--cantos-motion-feedback-duration);",
    ] {
        assert!(css.contains(declaration), "missing {declaration}");
    }
    let reduced = css
        .find("--cantos-web-search-open-duration: 0ms;")
        .expect("reduced-motion override");
    assert!(css[..reduced]
        .rfind("@media (prefers-reduced-motion: reduce)")
        .is_some());
    assert!(css.contains("--cantos-web-search-close-duration: 0ms;"));
}

fn rule_for<'a>(css: &'a str, selector_tail: &str) -> &'a str {
    let start = css
        .find(selector_tail)
        .unwrap_or_else(|| panic!("missing rule {selector_tail}"));
    let body = &css[start..];
    &body[..body.find('}').expect("closed rule")]
}

#[test]
fn the_guest_card_pair_and_account_control_read_scoped_contextual_roles_only() {
    let css = theatre_section();
    let card =
        ".cui-surface[data-tone=\"primary-container\"][data-variant=\"profile\"] .cui-button";
    // Sign-in: the contextual role with explicit hover/pressed fills and no state-layer gradient.
    let primary = rule_for(
        css,
        &format!("{card}[data-variant=\"auth-primary\"]:not(:disabled) {{"),
    );
    assert!(primary.contains("background-color: var(--cantos-color-theatre-profile-action);"));
    assert!(primary.contains("background-image: none;"));
    assert!(primary.contains("color: var(--cantos-color-theatre-on-profile-action);"));
    assert!(rule_for(
        css,
        &format!("{card}[data-variant=\"auth-primary\"]:hover:not(:disabled) {{")
    )
    .contains("var(--cantos-color-theatre-profile-action-hover)"));
    assert!(rule_for(
        css,
        &format!("{card}[data-variant=\"auth-primary\"]:active:not(:disabled) {{")
    )
    .contains("var(--cantos-color-theatre-profile-action-pressed)"));
    // Create account: the card's inverse pair, filled and never outlined.
    let secondary = rule_for(
        css,
        &format!("{card}[data-variant=\"auth-secondary\"]:not(:disabled) {{"),
    );
    assert!(secondary.contains("background-color: var(--cantos-color-on-primary-container);"));
    assert!(secondary.contains("color: var(--cantos-color-primary-container);"));
    assert!(!secondary.contains("border"), "{secondary}");
    // One full-width, equal-height anatomy; blocked actions keep the shared disabled colors.
    let shared = rule_for(
        css,
        &format!(
            "{card}:where([data-variant=\"auth-primary\"], [data-variant=\"auth-secondary\"]) {{"
        ),
    );
    assert!(shared.contains("inline-size: 100%;"));
    assert!(shared.contains("min-block-size: var(--cantos-size-auth-primary);"));
    let guest = css
        .split("/* Guest card actions")
        .nth(1)
        .and_then(|section| section.split_once("*/"))
        .expect("guest card section")
        .1;
    assert!(
        guest.matches(":not(:disabled)").count() >= 4,
        "blocked actions must not be recolored"
    );
    assert!(
        !guest.contains("filter:"),
        "contrast is a token decision, never a filter"
    );
    assert!(
        !guest.contains("auth-outlined"),
        "outlined actions keep their global roles"
    );
    // The theme action is a filled circle only inside an account tile, with an inset ink ring.
    let tile = ".cui-surface[data-variant=\"account-group\"] .cui-theme-cycle-button";
    let control = rule_for(css, &format!("{tile} {{"));
    assert!(control.contains("var(--cantos-color-theatre-account-control);"));
    assert!(control.contains("var(--cantos-color-theatre-on-account-control);"));
    assert!(rule_for(css, &format!("{tile}:where(:hover, :active) {{"))
        .contains("var(--cantos-color-theatre-account-control-hover)"));
    let focus = rule_for(css, &format!("{tile}:focus-visible {{"));
    assert!(focus.contains("outline-color: currentColor;"));
    assert!(focus.contains("outline-offset: calc(var(--cantos-size-focus-width) * -1);"));
    // Palette literals never enter the renderer template; both themes come from tokens.css.
    assert!(!guest.contains('#'));
}
