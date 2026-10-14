use leptos::prelude::*;

/// Token-backed spacing; renderer geometry and app pane rules stay outside this type.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Space {
    None,
    Xs,
    Sm,
    #[default]
    Md,
    Lg,
}

impl Space {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Xs => "xs",
            Self::Sm => "sm",
            Self::Md => "md",
            Self::Lg => "lg",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SurfaceTone {
    #[default]
    Plain,
    Container,
    High,
    Reading,
    PrimaryContainer,
}

impl SurfaceTone {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Plain => "plain",
            Self::Container => "container",
            Self::High => "high",
            Self::Reading => "reading",
            Self::PrimaryContainer => "primary-container",
        }
    }
}

/// Scoped auth/account geometry; regular listening/editor containment remains unchanged.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SurfaceVariant {
    #[default]
    Regular,
    Auth,
    Profile,
    Membership,
    AccountGroup,
}

impl SurfaceVariant {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Regular => "regular",
            Self::Auth => "auth",
            Self::Profile => "profile",
            Self::Membership => "membership",
            Self::AccountGroup => "account-group",
        }
    }
}

/// Quiet containment; callers supply semantic sections/headings through children.
#[component]
pub fn Surface(
    children: Children,
    #[prop(optional)] tone: SurfaceTone,
    #[prop(optional)] variant: SurfaceVariant,
    #[prop(default = Space::Md)] inset: Space,
    #[prop(optional, into)] class: Signal<String>,
) -> impl IntoView {
    view! {
        <div class=move || format!("cui-surface {}", class.get())
            data-tone=tone.as_str() data-variant=variant.as_str() data-inset=inset.as_str()>{children()}</div>
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StackDirection {
    #[default]
    Vertical,
    Horizontal,
}

impl StackDirection {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Vertical => "vertical",
            Self::Horizontal => "horizontal",
        }
    }
}

/// Horizontal stacks wrap; no child is squeezed below its text or native control minimum.
#[component]
pub fn Stack(
    children: Children,
    #[prop(optional)] direction: StackDirection,
    #[prop(optional)] gap: Space,
    #[prop(optional, into)] class: Signal<String>,
) -> impl IntoView {
    view! {
        <div class=move || format!("cui-stack {}", class.get())
            data-direction=direction.as_str() data-gap=gap.as_str()>{children()}</div>
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GridColumns {
    One,
    #[default]
    Two,
    Three,
}

impl GridColumns {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::One => "one",
            Self::Two => "two",
            Self::Three => "three",
        }
    }
}

/// Bounded responsive grid; column count is a maximum, based on available container width.
#[component]
pub fn Grid(
    children: Children,
    #[prop(optional)] columns: GridColumns,
    #[prop(optional)] gap: Space,
    #[prop(optional, into)] class: Signal<String>,
) -> impl IntoView {
    view! {
        <div class="cui-grid-container">
            <div class=move || format!("cui-grid {}", class.get())
                data-columns=columns.as_str() data-gap=gap.as_str()>{children()}</div>
        </div>
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ContainerWidth {
    Reading,
    #[default]
    Wide,
    Fluid,
}

impl ContainerWidth {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Reading => "reading",
            Self::Wide => "wide",
            Self::Fluid => "fluid",
        }
    }
}

#[component]
pub fn Container(
    children: Children,
    #[prop(optional)] width: ContainerWidth,
    #[prop(optional, into)] class: Signal<String>,
) -> impl IntoView {
    view! {
        <div class=move || format!("cui-container {}", class.get())
            data-width=width.as_str()>{children()}</div>
    }
}
