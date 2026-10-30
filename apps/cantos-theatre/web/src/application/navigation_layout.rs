//! Responsive presentation only; no route, session or player transition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NavigationLayout {
    Drawer,
    Compact,
    Expanded,
}

impl NavigationLayout {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Drawer => "drawer",
            Self::Compact => "compact",
            Self::Expanded => "expanded",
        }
    }
}

/// `*_fits` comes from actual, unwrapped localized text in the renderer's font.
pub(crate) fn navigation_layout(
    width: f64,
    wide_collapsed: bool,
    compact_fits: bool,
    expanded_fits: bool,
) -> NavigationLayout {
    if !width.is_finite() || width < 600.0 {
        NavigationLayout::Drawer
    } else if width >= 1200.0 && !wide_collapsed {
        if expanded_fits {
            NavigationLayout::Expanded
        } else {
            NavigationLayout::Drawer
        }
    } else if compact_fits {
        NavigationLayout::Compact
    } else {
        NavigationLayout::Drawer
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_text_uses_the_two_css_width_boundaries() {
        assert_eq!(NavigationLayout::Expanded.as_str(), "expanded");
        assert_eq!(NavigationLayout::Compact.as_str(), "compact");
        assert_eq!(NavigationLayout::Drawer.as_str(), "drawer");
        for (width, expected) in [
            (320.0, NavigationLayout::Drawer),
            (599.0, NavigationLayout::Drawer),
            (600.0, NavigationLayout::Compact),
            (700.0, NavigationLayout::Compact),
            (839.0, NavigationLayout::Compact),
            (840.0, NavigationLayout::Compact),
            (1024.0, NavigationLayout::Compact),
            (1199.0, NavigationLayout::Compact),
            (1200.0, NavigationLayout::Expanded),
            (1440.0, NavigationLayout::Expanded),
        ] {
            assert_eq!(navigation_layout(width, false, true, true), expected);
        }
    }

    #[test]
    fn wide_collapse_is_independent_of_medium_default_and_text_fit() {
        assert_eq!(
            navigation_layout(1440.0, true, true, true),
            NavigationLayout::Compact
        );
        assert_eq!(
            navigation_layout(1440.0, true, false, true),
            NavigationLayout::Drawer
        );
        assert_eq!(
            navigation_layout(1440.0, false, false, true),
            NavigationLayout::Expanded
        );
        assert_eq!(
            navigation_layout(600.0, false, false, true),
            NavigationLayout::Drawer
        );
        assert_eq!(
            navigation_layout(1440.0, false, true, false),
            NavigationLayout::Drawer
        );
    }

    #[test]
    fn unavailable_geometry_keeps_only_the_accessible_menu() {
        for width in [f64::NAN, f64::INFINITY, -1.0] {
            assert_eq!(
                navigation_layout(width, false, true, true),
                NavigationLayout::Drawer
            );
        }
    }
}
