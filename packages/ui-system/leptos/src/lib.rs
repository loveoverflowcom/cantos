//! Cantos Web foundation. Apps own localized content, routes, effects and permission state.

mod avatar;
mod button;
mod dialog;
mod feedback;
mod fields;
mod icons;
mod layout;
mod navigation;
mod root;
pub mod tokens;

pub use avatar::{
    avatar_initials, avatar_seed, avatar_tone, Avatar, AvatarInitials, AvatarSize, AvatarTone,
};
pub use button::{
    AccessibleName, Button, ButtonState, ButtonVariant, DisabledReason, EmptyAccessibleName,
    EmptyDisabledReason, IconButton,
};
pub use dialog::{Dialog, DialogCloseReason, DialogError};
pub use feedback::{Announcement, Badge, EmptyState, ErrorState, LoadingState, Status, StatusTone};
pub use fields::{
    Checkbox, ChoiceState, FieldError, FieldHelp, FieldLabel, FieldState, NumberField, Radio,
    RangeField, Select, Switch, TextArea, TextField, TextFieldKind,
};
pub use icons::{Icon, IconName};
pub use layout::{
    Container, ContainerWidth, Grid, GridColumns, Space, Stack, StackDirection, Surface,
    SurfaceTone,
};
pub use navigation::{FilterChip, InvalidLinkHref, Link, LinkHref, NavItem, NavTabs};
pub use root::{Density, Theme, UiRoot};

/// Load these before app styles. Trunk consumers can link the same files directly.
pub const TOKEN_CSS: &str = include_str!("../tokens.css");
pub const FOUNDATION_CSS: &str = include_str!("../foundation.css");
