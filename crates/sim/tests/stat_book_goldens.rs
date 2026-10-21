//! Source stat books remain owned values through authored overrides and saves.

#[cfg(feature = "persistence")]
#[test]
fn stock_stat_book_and_player_overrides_retain_their_source_rows() {
    use solomon_sim::profile::{NativeSkillStatBookValue, PlayerStatBookComponent};

    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/website/profile-goldens.json")).unwrap();
    let expected: Vec<solomon_sim::profile::NativeSkillStatBookEntry> =
        serde_json::from_value(fixture["statBook"].clone()).unwrap();
    let mut book = PlayerStatBookComponent::default();
    assert_eq!(book.entries, expected);

    book.entries[8].description = "Authored missile damage".into();
    book.entries[8]
        .numeric_properties
        .insert("mDamage1".into(), NativeSkillStatBookValue::Ranks(vec![0.0, 17.25, 31.5]));
    book.entries[8]
        .numeric_properties
        .insert("mManaCost".into(), NativeSkillStatBookValue::Scalar(0.75));
    assert_eq!(book.stat(8, "mDamage1", 1), Some(17.25));
    assert_eq!(book.stat(8, "mDamage1", 20), Some(31.5));
    assert_eq!(book.stat(8, "mManaCost", 0), Some(0.75));
    let encoded = serde_json::to_string(&book).unwrap();
    let restored: PlayerStatBookComponent = serde_json::from_str(&encoded).unwrap();
    assert_eq!(restored, book);
    assert_ne!(restored, PlayerStatBookComponent::default());
}
