use std::cmp::Ordering;

use icu_calendar::DateTime;
use icu_collator::{Collator, CollatorBorrowed, options::CollatorOptions};
use icu_datetime::{DateTimeFormatter, options::components};
use icu_decimal::DecimalFormatter;
use icu_locale::Locale;
use time::{OffsetDateTime, UtcOffset, format_description::well_known::Rfc3339};
use writeable::Writeable;

struct Formatting {
    locale: Locale,
    collator: CollatorBorrowed<'static>,
    decimal: DecimalFormatter,
    date: DateTimeFormatter,
}

impl Formatting {
    fn new(locale: Locale) -> Self {
        let mut date_options = components::Bag::default();
        date_options.year = Some(components::Year::Numeric);
        date_options.month = Some(components::Month::Short);
        date_options.day = Some(components::Day::NumericDayOfMonth);
        Self {
            collator: Collator::try_new((&locale).into(), CollatorOptions::default())
                .expect("compiled ICU data includes root collation"),
            decimal: DecimalFormatter::try_new((&locale).into(), Default::default())
                .expect("compiled ICU data includes root decimal formatting"),
            date: DateTimeFormatter::try_new_experimental(
                &locale.to_string().parse().expect("valid ICU locale"),
                date_options.into(),
            )
            .expect("compiled ICU data includes root calendar formatting"),
            locale,
        }
    }

    fn date_at(&self, date: OffsetDateTime) -> Result<String, String> {
        let date = DateTime::try_new_iso_datetime(
            date.year(),
            u8::from(date.month()),
            date.day(),
            0,
            0,
            0,
        )
        .map_err(|error| format!("Cannot format this date: {error}"))?;
        self.date
            .format_to_string(&date.to_any())
            .map_err(|error| format!("Cannot format this date: {error}"))
    }
}

thread_local! {
    static FORMATTING: Formatting = Formatting::new(sys_locale::get_locale()
        .and_then(|locale| locale.parse().ok()).unwrap_or(icu_locale::locale!("und")));
}

pub fn compare(first: &str, second: &str) -> Ordering {
    FORMATTING.with(|formatting| formatting.collator.compare(first, second))
}

pub fn lowercase(value: &str) -> String {
    FORMATTING.with(|formatting| {
        icu_casemap::CaseMapper::new()
            .lowercase_to_string(value, &formatting.locale.id)
            .into_owned()
    })
}

pub fn integer(value: u64) -> Result<String, String> {
    Ok(FORMATTING
        .with(|formatting| formatting.decimal.format(&value.into()).write_to_string().into_owned()))
}

pub fn timestamp(value: &str) -> Option<f64> {
    OffsetDateTime::parse(value, &Rfc3339)
        .ok()
        .map(|date| date.unix_timestamp_nanos() as f64 / 1_000_000.0)
}

pub fn date(value: &str) -> Result<String, String> {
    let Ok(date) = OffsetDateTime::parse(value, &Rfc3339) else {
        return Ok(String::new());
    };
    let offset = UtcOffset::local_offset_at(date)
        .map_err(|error| format!("Cannot read the local timezone: {error}"))?;
    FORMATTING.with(|formatting| formatting.date_at(date.to_offset(offset)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_metadata_respects_locale_and_local_calendar_day() {
        let english = Formatting::new("en-US".parse().unwrap());
        let german = Formatting::new("de-DE".parse().unwrap());
        assert_eq!(english.decimal.format(&1234_u32.into()).write_to_string(), "1,234");
        assert_eq!(german.decimal.format(&1234_u32.into()).write_to_string(), "1.234");
        let utc = OffsetDateTime::parse("2026-10-26T02:00:00Z", &Rfc3339).unwrap();
        let local = utc.to_offset(UtcOffset::from_hms(-4, 0, 0).unwrap());
        assert_eq!(english.date_at(local).unwrap(), "Oct 25, 2026");
        assert_eq!(german.date_at(local).unwrap(), "25. Okt. 2026");
        for (locale, expected) in [
            ("es-ES", "25 oct 2026"),
            ("ja-JP", "2026年10月25日"),
            ("ar-EG", "٢٥ أكتوبر ٢٠٢٦"),
            ("th-TH", "25 ต.ค. 2569"),
        ] {
            assert_eq!(Formatting::new(locale.parse().unwrap()).date_at(local).unwrap(), expected);
        }
        assert_eq!(english.collator.compare("a", "B"), Ordering::Less);
        assert_eq!(
            icu_casemap::CaseMapper::new().lowercase_to_string("Iİ", &"tr".parse().unwrap()),
            "ıi"
        );
        assert_eq!(timestamp("2026-10-26T02:00:00Z"), Some(1792980000000.0));
        assert!(timestamp("not a date").is_none());
    }
}
