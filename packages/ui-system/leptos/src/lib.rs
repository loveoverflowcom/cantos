//! Cantos Web foundation. Apps own localized content, routes, effects and permission state.

mod button;
mod icons;
mod root;
pub mod tokens;

pub use button::{Button, ButtonState, ButtonVariant, DisabledReason, EmptyDisabledReason};
pub use icons::{Icon, IconName};
pub use root::{Density, Theme, UiRoot};

/// Load these before app styles. Trunk consumers can link the same files directly.
pub const TOKEN_CSS: &str = include_str!("../tokens.css");
pub const FOUNDATION_CSS: &str = include_str!("../foundation.css");
