use leptos::prelude::*;

/// System follows the browser preference in CSS; it is not a resolved color palette.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

impl Theme {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }

    pub fn from_attribute(value: &str) -> Option<Self> {
        match value {
            "system" => Some(Self::System),
            "light" => Some(Self::Light),
            "dark" => Some(Self::Dark),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Density {
    #[default]
    Compact,
    Comfortable,
}

impl Density {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Compact => "compact",
            Self::Comfortable => "comfortable",
        }
    }
}

/// An opt-in remap of semantic roles for one subtree (token v0.7.0). Standard keeps every
/// consumer's roles, so Studio and any app that names no expression render exactly as before;
/// Theatre selects the expressive palette and bold weights from the same canonical source.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Expression {
    #[default]
    Standard,
    Theatre,
}

impl Expression {
    /// The generated [`crate::tokens::EXPRESSIONS`] name, or `None` for Standard, which has no scope.
    pub const fn attribute(self) -> Option<&'static str> {
        match self {
            Self::Standard => None,
            Self::Theatre => Some("theatre"),
        }
    }
}

/// A scoped theme boundary, with no route, global stylesheet injection or browser storage.
/// The expression scope is a `data-expression` attribute and appears only when one is chosen.
#[component]
pub fn UiRoot(
    children: Children,
    #[prop(into, default = Theme::System.into())] theme: Signal<Theme>,
    #[prop(into, default = Density::Compact.into())] density: Signal<Density>,
    #[prop(into, default = Expression::Standard.into())] expression: Signal<Expression>,
    #[prop(optional, into)] class: String,
) -> impl IntoView {
    view! {
        <div class=format!("cantos-ui {class}")
            data-theme=move || theme.get().as_str()
            data-density=move || density.get().as_str()
            data-expression=move || expression.get().attribute()>
            {children()}
        </div>
    }
}
