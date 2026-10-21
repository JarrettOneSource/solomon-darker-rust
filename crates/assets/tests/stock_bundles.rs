//! Parses the tracked retail bundles and checks their record counts against the counts the
//! Website extractor asserts. `SOLOMON_STOCK_DIR` selects an external retail root's images.

use std::path::PathBuf;

use solomon_assets::{SpriteRecord, parse_bundle};

fn stock_images_dir() -> PathBuf {
    std::env::var("SOLOMON_STOCK_DIR")
        .map(|root| PathBuf::from(root).join("images"))
        .unwrap_or_else(|_| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../web/assets/stock")
        })
}

fn stock_records(name: &str) -> Vec<SpriteRecord> {
    let path = stock_images_dir().join(name);
    let bytes = std::fs::read(&path)
        .unwrap_or_else(|error| panic!("stock bundle {} is required: {error}", path.display()));
    parse_bundle(&bytes).unwrap_or_else(|error| panic!("stock bundle {}: {error}", path.display()))
}

#[test]
fn every_stock_bundle_parses_to_its_known_record_count() {
    let expected = [
        ("BadGuys.bundle", 2509),
        ("Clothes.bundle", 3724),
        ("College.bundle", 543),
        ("Inventory.bundle", 84),
        ("LevelPicker.bundle", 8),
        ("Library.bundle", 33),
        ("Memoratorium.bundle", 76),
        ("Office.bundle", 27),
        ("Skills.bundle", 166),
        ("Solomon.bundle", 273),
        ("Storage.bundle", 27),
        ("UI.bundle", 113),
    ];
    for (name, count) in expected {
        assert_eq!(stock_records(name).len(), count, "{name} record count");
    }
}

#[test]
fn the_wardrobe_bundle_carries_the_wizard_layer_banks() {
    let records = stock_records("Clothes.bundle");
    // Staff_RenderAttachment reads points 1 and 2 of records 3244 + pose * 24 + heading.
    for pose in 0..10 {
        for heading in 0..24 {
            let record = &records[3244 + pose * 24 + heading];
            assert_eq!(
                record.points.len(),
                3,
                "staff attachment pose {pose} heading {heading} endpoints"
            );
            assert_eq!((record.logical_width, record.logical_height), (170, 170));
        }
    }
    // Dynamic robes (868 primary, 1228 secondary) register into 150-unit cells; the fixed
    // robes (1612/2428 primary, 2020/2836 secondary) register into the 170-unit player cell.
    for base in [868, 1228] {
        for (offset, record) in records[base..base + 5 * 24].iter().enumerate() {
            let index = base + offset;
            assert_eq!(
                (record.logical_width, record.logical_height),
                (150, 150),
                "record {index} cell"
            );
            assert!(record.width > 0 && record.height > 0, "record {index} is empty");
        }
    }
    for base in [1612, 2428, 2020, 2836] {
        for (offset, record) in records[base..base + 10 * 24].iter().enumerate() {
            let index = base + offset;
            assert_eq!(
                (record.logical_width, record.logical_height),
                (170, 170),
                "record {index} cell"
            );
        }
    }
    for index in [316, 316 + 23, 412, 412 + 23] {
        assert_eq!(
            (records[index].logical_width, records[index].logical_height),
            (150, 150),
            "head record {index} cell"
        );
    }
}
