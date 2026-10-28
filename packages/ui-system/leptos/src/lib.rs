//! Cantos Web foundation. Apps own localized content, routes, effects and permission state.

mod avatar;
mod back;
mod button;
mod dialog;
mod feedback;
mod fields;
mod icons;
mod layout;
mod logo;
mod navigation;
mod root;
mod select_menu;
mod theme_cycle;
pub mod tokens;
mod tooltip;

pub use avatar::{
    avatar_image_src, avatar_initials, avatar_seed, avatar_tone, Avatar, AvatarInitials,
    AvatarSize, AvatarTone,
};
pub use back::{BackButton, BackButtonVariant};
pub use button::{
    AccessibleName, Button, ButtonState, ButtonVariant, DisabledReason, EmptyAccessibleName,
    EmptyDisabledReason, IconButton,
};
pub use dialog::{Dialog, DialogCloseReason, DialogError};
pub use feedback::{
    Announcement, Badge, EdgeProgress, EmptyState, ErrorState, LoadingState, Status, StatusTone,
    SurfacePulse,
};
pub use fields::{
    Checkbox, ChoiceState, FieldDensity, FieldError, FieldHelp, FieldLabel, FieldState,
    NumberField, Radio, RangeField, SearchDock, Select, SelectDensity, SelectLabelPlacement,
    Switch, TextArea, TextField, TextFieldKind, TextFieldVariant,
};
pub use icons::{Icon, IconName};
pub use layout::{
    Container, ContainerWidth, Grid, GridColumns, Space, Stack, StackDirection, Surface,
    SurfaceTone, SurfaceVariant,
};
pub use logo::{Logo, LogoTone, LogoVariant};
pub use navigation::{FilterChip, InvalidLinkHref, Link, LinkHref, NavItem, NavTabs};
pub use root::{Density, Expression, Theme, UiRoot};
pub use select_menu::SelectOption;
pub use theme_cycle::{ThemeCycleButton, ThemeCycleLabels};
pub use tooltip::Tooltip;

/// Load these before app styles. Trunk consumers can link the same files directly.
pub const TOKEN_CSS: &str = include_str!("../tokens.css");
pub const FOUNDATION_CSS: &str = include_str!("../foundation.css");
