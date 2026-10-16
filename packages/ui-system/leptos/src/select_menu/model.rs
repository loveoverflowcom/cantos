//! Select-only combobox decisions. Browsing never acknowledges a caller-owned value.
use super::SelectOption;

/// Web-local typeahead interval, in monotonic DOM event milliseconds; not a motion duration.
const TYPEAHEAD_INTERVAL_MS: f64 = 700.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Intent {
    Toggle,
    Next,
    Previous,
    First,
    Last,
    Accept,
    Dismiss,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct MenuState {
    pub open: bool,
    pub active: Option<usize>,
    prefix: String,
    typed_at: f64,
}

impl MenuState {
    fn initial(options: &[SelectOption], value: &str) -> Option<usize> {
        options
            .iter()
            .position(|option| option.value == value && !option.disabled)
            .or_else(|| options.iter().position(|option| !option.disabled))
    }

    pub fn apply(
        &mut self,
        intent: Intent,
        options: &[SelectOption],
        value: &str,
    ) -> Option<String> {
        let was_open = self.open;
        if matches!(intent, Intent::Dismiss) || (was_open && intent == Intent::Toggle) {
            *self = Self::default();
            return None;
        }
        if was_open && intent == Intent::Accept {
            let committed = self
                .active
                .and_then(|index| options.get(index))
                .filter(|option| !option.disabled)
                .map(|option| option.value.clone());
            *self = Self::default();
            return committed;
        }
        if !was_open {
            self.open = true;
            self.active = Self::initial(options, value);
        }
        self.active = match intent {
            Intent::First => options.iter().position(|option| !option.disabled),
            Intent::Last => options.iter().rposition(|option| !option.disabled),
            Intent::Next if was_open => options
                .iter()
                .enumerate()
                .find(|(index, option)| {
                    !option.disabled && self.active.is_none_or(|active| *index > active)
                })
                .map(|(index, _)| index)
                .or(self.active),
            Intent::Previous if was_open => options
                .iter()
                .enumerate()
                .rev()
                .find(|(index, option)| {
                    !option.disabled && self.active.is_none_or(|active| *index < active)
                })
                .map(|(index, _)| index)
                .or(self.active),
            _ => self.active,
        };
        self.prefix.clear();
        None
    }

    pub fn type_character(
        &mut self,
        character: char,
        at: f64,
        options: &[SelectOption],
        value: &str,
    ) {
        if !self.open {
            self.open = true;
            self.active = Self::initial(options, value);
        }
        let character = character.to_lowercase().collect::<String>();
        let repeated = self.prefix == character;
        if at - self.typed_at > TYPEAHEAD_INTERVAL_MS || repeated {
            self.prefix.clear();
        }
        self.prefix.push_str(&character);
        self.typed_at = at;
        let start = if repeated {
            self.active.map_or(0, |index| index + 1)
        } else {
            0
        };
        self.active = (start..options.len())
            .chain(0..start.min(options.len()))
            .find(|index| {
                !options[*index].disabled
                    && options[*index]
                        .label
                        .to_lowercase()
                        .starts_with(&self.prefix)
            })
            .or(self.active);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options() -> Vec<SelectOption> {
        vec![
            SelectOption::new("system", "Theo hệ thống"),
            SelectOption::new("light", "Sáng"),
            SelectOption {
                disabled: true,
                ..SelectOption::new("blocked", "Không khả dụng")
            },
            SelectOption::new("dark", "Tối"),
        ]
    }

    #[test]
    fn browsing_skips_disabled_without_changing_value_and_escape_discards_it() {
        let mut state = MenuState::default();
        let options = options();
        for intent in [Intent::Toggle, Intent::Next, Intent::Next] {
            assert_eq!(state.apply(intent, &options, "system"), None);
        }
        assert_eq!(state.active, Some(3));
        assert_eq!(state.apply(Intent::Next, &options, "system"), None);
        assert_eq!(state.active, Some(3));
        assert_eq!(state.apply(Intent::Dismiss, &options, "system"), None);
        assert_eq!(state, MenuState::default());
        state.apply(Intent::Toggle, &options, "system");
        assert_eq!(state.active, Some(0));
    }

    #[test]
    fn acceptance_is_explicit_and_revalidates_removed_or_disabled_options() {
        let mut state = MenuState::default();
        let options = options();
        state.apply(Intent::Last, &options, "system");
        assert_eq!(
            state.apply(Intent::Accept, &options, "system"),
            Some("dark".to_owned())
        );
        state.apply(Intent::Last, &options, "system");
        assert_eq!(state.apply(Intent::Accept, &[], "system"), None);
        state.apply(Intent::First, &options, "missing");
        let blocked = options
            .into_iter()
            .map(|option| SelectOption {
                disabled: true,
                ..option
            })
            .collect::<Vec<_>>();
        assert_eq!(state.apply(Intent::Accept, &blocked, "missing"), None);
    }

    #[test]
    fn missing_value_and_empty_options_never_dispatch_a_default() {
        for options in [options(), vec![]] {
            let mut state = MenuState::default();
            assert_eq!(state.apply(Intent::Toggle, &options, "missing"), None);
            assert_eq!(state.apply(Intent::Dismiss, &options, "missing"), None);
        }
    }

    #[test]
    fn home_end_previous_and_typeahead_use_enabled_localized_labels() {
        let options = options();
        let mut state = MenuState::default();
        state.apply(Intent::Last, &options, "system");
        state.apply(Intent::Previous, &options, "system");
        assert_eq!(state.active, Some(1));
        state.apply(Intent::First, &options, "system");
        assert_eq!(state.active, Some(0));
        state.type_character('T', 1.0, &options, "system");
        assert_eq!(state.active, Some(0));
        state.type_character('t', 2.0, &options, "system");
        assert_eq!(state.active, Some(3));
        state.type_character('s', 1000.0, &options, "system");
        state.type_character('á', 1001.0, &options, "system");
        assert_eq!(state.active, Some(1));
    }
}
