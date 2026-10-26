use cantos_localization::{Currency, Formatter, TimeZone};

fn main() {
    for tag in ["vi-VN", "en", "ja"] {
        let formatter = Formatter::try_new(tag).expect("probe locale");
        for units in [0, 1, 1234] {
            println!(
                "{tag} USD {units}: {:?}",
                formatter.currency(units, Currency::Usd)
            );
            println!(
                "{tag} VND {units}: {:?}",
                formatter.currency(units, Currency::Vnd)
            );
        }
        for instant in [-1, 0, 86_400_000, 1_700_000_000_000] {
            println!(
                "{tag} UTC {instant}: {:?}",
                formatter.date_time(instant, TimeZone::Utc)
            );
        }
        println!(
            "{tag} UTC+7: {:?}",
            formatter.date_time(0, TimeZone::OffsetMinutes(420))
        );
        println!(
            "{tag} clock: {} {} {}",
            formatter.clock(0),
            formatter.clock(3599),
            formatter.clock(3600)
        );
    }
}
