//! Presentation routes only. Demo IDs are not publication or API identifiers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Route {
    Discover,
    Explore,
    Library,
    Work,
    Player,
    NotFound,
}

impl Route {
    pub fn from_hash(hash: &str) -> Self {
        match hash {
            "" | "#" | "#/" | "#/discover" => Self::Discover,
            "#/explore" => Self::Explore,
            "#/library" => Self::Library,
            "#/work/demo-son-ha" => Self::Work,
            "#/player/demo-episode" => Self::Player,
            _ => Self::NotFound,
        }
    }

    pub const fn hash(self) -> &'static str {
        match self {
            Self::Discover => "#/discover",
            Self::Explore => "#/explore",
            Self::Library => "#/library",
            Self::Work => "#/work/demo-son-ha",
            Self::Player => "#/player/demo-episode",
            Self::NotFound => "#/not-found",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DemoState {
    #[default]
    Ready,
    Loading,
    Empty,
    Error,
    NotImplemented,
}

impl DemoState {
    pub const fn control_value(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Loading => "loading",
            Self::Empty => "empty",
            Self::Error => "error",
            Self::NotImplemented => "not-implemented",
        }
    }

    pub fn from_control(value: &str) -> Self {
        match value {
            "loading" => Self::Loading,
            "empty" => Self::Empty,
            "error" => Self::Error,
            "not-implemented" => Self::NotImplemented,
            _ => Self::Ready,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_listener_demo_routes_resolve_and_deep_links_round_trip() {
        for route in [
            Route::Discover,
            Route::Explore,
            Route::Library,
            Route::Work,
            Route::Player,
        ] {
            assert_eq!(Route::from_hash(route.hash()), route);
        }
        for invalid in [
            "#/studio",
            "#/api/v1/scripts",
            "#/work/private",
            "#/player/real-episode",
            "#/work/demo-son-ha/extra",
        ] {
            assert_eq!(Route::from_hash(invalid), Route::NotFound);
        }
        assert_eq!(Route::from_hash(""), Route::Discover);
    }

    #[test]
    fn failure_and_missing_boundary_are_distinct_from_empty() {
        assert_eq!(DemoState::from_control("error"), DemoState::Error);
        assert_eq!(DemoState::from_control("empty"), DemoState::Empty);
        assert_eq!(
            DemoState::from_control("not-implemented"),
            DemoState::NotImplemented
        );
    }
}
