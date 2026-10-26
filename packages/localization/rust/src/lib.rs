//! Renderer-neutral ICU4X formatting. Catalogs and generated message types stay app-owned.
use icu_decimal::{input::Decimal, options::DecimalFormatterOptions, DecimalFormatter};
use icu_locale::Locale;
use icu_plurals::PluralRules;

pub use icu_plurals::PluralCategory;

/// Compiled ICU4X 2.3 data derives from CLDR 48.2.1; Cargo.lock binds exact data crate bytes.
pub const ICU4X_LINE: &str = "2.3";
pub const CLDR_VERSION: &str = "48.2.1";

pub fn canonical_locale(tag: &str) -> Result<String, FormatError> {
    tag.parse::<Locale>()
        .map(|locale| locale.to_string())
        .map_err(|_| FormatError::InvalidLocale)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FormatError {
    InvalidLocale,
    InvalidDecimal,
    DataUnavailable,
    UnsupportedCurrency,
    InvalidInstant,
    UnsupportedTimeZone,
    OutOfRange,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Currency {
    Usd,
    Vnd,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimeZone<'a> {
    Utc,
    OffsetMinutes(i16),
    Iana(&'a str),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreferenceWarning {
    Unsupported,
    StorageUnavailable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StoredPreference<'a> {
    Missing,
    Saved(&'a str),
    Unavailable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PreferenceSelection<L> {
    pub locale: L,
    pub warning: Option<PreferenceWarning>,
}

/// No device/browser negotiation. Applying the default after an unsupported saved tag is visible.
pub fn select_preference<L>(
    stored: StoredPreference<'_>,
    default: L,
    resolve: impl FnOnce(&str) -> Option<L>,
) -> PreferenceSelection<L> {
    match stored {
        StoredPreference::Saved(tag) => match resolve(tag) {
            Some(locale) => PreferenceSelection {
                locale,
                warning: None,
            },
            None => PreferenceSelection {
                locale: default,
                warning: Some(PreferenceWarning::Unsupported),
            },
        },
        StoredPreference::Missing => PreferenceSelection {
            locale: default,
            warning: None,
        },
        StoredPreference::Unavailable => PreferenceSelection {
            locale: default,
            warning: Some(PreferenceWarning::StorageUnavailable),
        },
    }
}

/// Locale parsing is separate from the application's supported resource registry.
pub struct Formatter {
    locale: Locale,
    decimal: DecimalFormatter,
    cardinal: PluralRules,
}

impl Formatter {
    pub fn try_new(tag: &str) -> Result<Self, FormatError> {
        let locale = tag
            .parse::<Locale>()
            .map_err(|_| FormatError::InvalidLocale)?;
        let mut options = DecimalFormatterOptions::default();
        options.grouping_strategy = Some(icu_decimal::options::GroupingStrategy::Never);
        Ok(Self {
            locale: locale.clone(),
            decimal: DecimalFormatter::try_new(locale.clone().into(), options)
                .map_err(|_| FormatError::DataUnavailable)?,
            cardinal: PluralRules::try_new_cardinal(locale.into())
                .map_err(|_| FormatError::DataUnavailable)?,
        })
    }

    /// An exact decimal string avoids binary floating-point and preserves visible fraction digits.
    pub fn number(&self, value: &str) -> Result<String, FormatError> {
        let value = value
            .parse::<Decimal>()
            .map_err(|_| FormatError::InvalidDecimal)?;
        Ok(self.decimal.format(&value).to_string())
    }

    pub fn cardinal_categories(&self) -> impl Iterator<Item = PluralCategory> + '_ {
        self.cardinal.categories()
    }

    pub fn cardinal(&self, value: &str) -> Result<PluralCategory, FormatError> {
        let value = value
            .parse::<Decimal>()
            .map_err(|_| FormatError::InvalidDecimal)?;
        Ok(self.cardinal.category_for(&value))
    }

    pub fn integer(&self, value: u64, minimum_digits: i16) -> String {
        let mut value = Decimal::from(value);
        value.pad_start(minimum_digits);
        self.decimal.format(&value).to_string()
    }

    pub fn hundredths(&self, value: u16) -> String {
        let mut value = Decimal::from(value);
        value.multiply_pow10(-2);
        value.trim_end();
        self.decimal.format(&value).to_string()
    }

    /// Qualified numeric player clock: H:mm:ss after the hour boundary, otherwise m:ss.
    /// Only punctuation is fixed; all digits and padding pass through ICU4X.
    pub fn clock(&self, seconds: u64) -> String {
        let second = self.integer(seconds % 60, 2);
        if seconds < 3600 {
            format!("{}:{second}", self.integer(seconds / 60, 1))
        } else {
            format!(
                "{}:{}:{second}",
                self.integer(seconds / 3600, 1),
                self.integer(seconds / 60 % 60, 2)
            )
        }
    }

    pub fn currency(&self, minor_units: u64, currency: Currency) -> Result<String, FormatError> {
        use icu_experimental::dimension::currency::{formatter::CurrencyFormatter, CurrencyType};
        let (code, scale) = match currency {
            Currency::Usd => ("USD", -2),
            Currency::Vnd => ("VND", 0),
        };
        let code = code
            .parse::<CurrencyType>()
            .map_err(|_| FormatError::UnsupportedCurrency)?;
        let formatter =
            CurrencyFormatter::try_new_code(self.locale.clone().into(), code, Default::default())
                .map_err(|_| FormatError::DataUnavailable)?;
        let mut value = Decimal::from(minor_units);
        value.multiply_pow10(scale);
        let formatted = formatter.format_fixed_decimal(&value).to_string();
        Ok(formatted)
    }

    /// Explicit UTC/fixed-offset instant request. IANA rule lookup is not qualified here.
    pub fn date_time(&self, unix_ms: i64, zone: TimeZone<'_>) -> Result<String, FormatError> {
        use icu_datetime::{
            fieldsets,
            input::{Date, DateTime, Time},
            DateTimeFormatter,
        };
        let offset = match zone {
            TimeZone::Utc => 0,
            TimeZone::OffsetMinutes(minutes) if minutes.unsigned_abs() <= 1080 => minutes,
            _ => return Err(FormatError::UnsupportedTimeZone),
        };
        let seconds = unix_ms
            .div_euclid(1000)
            .checked_add(i64::from(offset) * 60)
            .ok_or(FormatError::InvalidInstant)?;
        let (days, time) = (seconds.div_euclid(86_400), seconds.rem_euclid(86_400));
        // Gregorian civil-from-days conversion; formatting itself belongs entirely to ICU4X.
        let z = days + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z.rem_euclid(146_097);
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let day = doy - (153 * mp + 2) / 5 + 1;
        let month = if mp < 10 { mp + 3 } else { mp - 9 };
        let year = i32::try_from(yoe + era * 400 + i64::from(month <= 2))
            .map_err(|_| FormatError::InvalidInstant)?;
        let input = DateTime {
            date: Date::try_new_iso(year, month as u8, day as u8)
                .map_err(|_| FormatError::InvalidInstant)?,
            time: Time::try_new(
                (time / 3600) as u8,
                (time % 3600 / 60) as u8,
                (time % 60) as u8,
                0,
            )
            .map_err(|_| FormatError::InvalidInstant)?,
        };
        let locale = format!("{}-u-hc-h23", self.locale)
            .parse::<Locale>()
            .map_err(|_| FormatError::InvalidLocale)?;
        let formatter =
            DateTimeFormatter::try_new(locale.into(), fieldsets::YMD::medium().with_time_hm())
                .map_err(|_| FormatError::DataUnavailable)?;
        let zone = if offset == 0 {
            "UTC".to_owned()
        } else {
            let magnitude = offset.unsigned_abs();
            format!(
                "UTC{}{:02}:{:02}",
                if offset < 0 { '-' } else { '+' },
                magnitude / 60,
                magnitude % 60
            )
        };
        Ok(format!("{} {zone}", formatter.format(&input)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pinned_icu_decimal_and_fractional_plural_vectors_are_exact() {
        for (tag, decimal, category) in [
            ("vi-VN", "1234,50", PluralCategory::Other),
            ("en", "1234.50", PluralCategory::Other),
            ("ja", "1234.50", PluralCategory::Other),
        ] {
            let formatter = Formatter::try_new(tag).unwrap();
            assert_eq!(formatter.number("1234.50"), Ok(decimal.into()));
            assert_eq!(formatter.cardinal("1.0"), Ok(category));
        }
        let english = Formatter::try_new("en").unwrap();
        for (value, category) in [
            ("0", PluralCategory::Other),
            ("1", PluralCategory::One),
            ("2", PluralCategory::Other),
        ] {
            assert_eq!(english.cardinal(value), Ok(category));
        }
        let arabic = Formatter::try_new("ar").unwrap();
        assert_eq!(arabic.cardinal("2"), Ok(PluralCategory::Two));
        assert_eq!(arabic.cardinal("3"), Ok(PluralCategory::Few));
    }

    #[test]
    fn invalid_inputs_are_typed_refusals() {
        assert!(matches!(
            Formatter::try_new("not a locale"),
            Err(FormatError::InvalidLocale)
        ));
        let formatter = Formatter::try_new("vi-VN").unwrap();
        assert_eq!(formatter.number("NaN"), Err(FormatError::InvalidDecimal));
    }

    #[test]
    fn bounded_money_clock_and_explicit_zone_requests_match_reviewed_vectors() {
        let vi = Formatter::try_new("vi-VN").unwrap();
        let en = Formatter::try_new("en").unwrap();
        assert_eq!(
            vi.currency(1234, Currency::Usd),
            Ok("12,34\u{a0}USD".into())
        );
        assert_eq!(
            en.currency(1234, Currency::Usd),
            Ok("USD\u{a0}12.34".into())
        );
        assert_eq!(
            vi.currency(1234, Currency::Vnd),
            Ok("1.234\u{a0}VND".into())
        );
        assert_eq!(
            en.currency(1234, Currency::Vnd),
            Ok("VND\u{a0}1,234".into())
        );
        assert_eq!(en.currency(0, Currency::Usd), Ok("USD\u{a0}0.00".into()));
        assert_eq!(
            en.currency(u64::MAX, Currency::Usd),
            Ok("USD\u{a0}184,467,440,737,095,516.15".into())
        );
        assert_eq!(
            vi.date_time(0, TimeZone::Utc),
            Ok("00:00 1 thg 1, 1970 UTC".into())
        );
        assert_eq!(
            en.date_time(-1, TimeZone::Utc),
            Ok("Dec 31, 1969, 23:59 UTC".into())
        );
        assert_eq!(
            en.date_time(0, TimeZone::OffsetMinutes(420)),
            Ok("Jan 1, 1970, 07:00 UTC+07:00".into())
        );
        assert_eq!(
            en.date_time(0, TimeZone::OffsetMinutes(-60)),
            Ok("Dec 31, 1969, 23:00 UTC-01:00".into())
        );
        assert_eq!(
            en.date_time(i64::MAX, TimeZone::Utc),
            Err(FormatError::InvalidInstant)
        );
        assert_eq!(
            en.date_time(0, TimeZone::OffsetMinutes(1081)),
            Err(FormatError::UnsupportedTimeZone)
        );
        assert_eq!(
            en.date_time(0, TimeZone::Iana("Asia/Ho_Chi_Minh")),
            Err(FormatError::UnsupportedTimeZone)
        );
        for (seconds, expected) in [
            (0, "0:00"),
            (61, "1:01"),
            (3599, "59:59"),
            (3600, "1:00:00"),
            (3723, "1:02:03"),
        ] {
            assert_eq!(en.clock(seconds), expected);
            assert_eq!(vi.clock(seconds), expected);
        }
        assert_eq!(vi.hundredths(150), "1,5");
        assert_eq!(en.hundredths(75), "0.75");
        assert_eq!(en.hundredths(100), "1");
    }
}
