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

/// A scoped theme boundary, with no route, global stylesheet injection or browser storage.
#[component]
pub fn UiRoot(
    children: Children,
    #[prop(into, default = Theme::System.into())] theme: Signal<Theme>,
    #[prop(into, default = Density::Compact.into())] density: Signal<Density>,
    #[prop(optional, into)] class: String,
) -> impl IntoView {
    view! {
        <div class=format!("cantos-ui {class}")
            data-theme=move || theme.get().as_str()
            data-density=move || density.get().as_str()>
            {children()}
        </div>
    }
}
