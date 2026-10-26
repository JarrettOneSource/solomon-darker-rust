//! The browser journey imports unchanged Source-produced inventory fixtures.
//! This validates their real file-selection boundary before browser admission.
use serde_json::Value;
use solomon_assets::native_save::{
    GameSaveImportSource, GameSaveInputFile, read_game_save_file_selection,
};
use solomon_assets::website_save;
use solomon_sim::economy::{HubItemKind, NativeSkillBookFeedbackCursor};

#[test]
fn source_book_fixtures_import_owned_books_and_seed_their_retained_receipt() {
    let bytes = miniz_oxide::inflate::decompress_to_vec_zlib(include_bytes!(
        "../../sim/tests/fixtures/website/skill-book-browser-saves.json.zlib"
    ))
    .unwrap();
    let fixture: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(fixture["sourceRevision"], "fa08ae249ff5563570bdb2a653e2ef9358ce7b83");
    for case in fixture["cases"].as_array().unwrap() {
        let document = case["document"].to_string();
        let imported = read_game_save_file_selection(&[GameSaveInputFile {
            name: "source-books.json",
            relative_path: "",
            bytes: document.as_bytes(),
        }])
        .unwrap_or_else(|error| panic!("{}: {error}", case["world"]));
        assert_eq!(imported.source, GameSaveImportSource::Browser);
        let restored = website_save::restore(&imported.document).unwrap();
        assert_eq!(restored.session.boneyard.is_some(), case["world"] == "boneyard");
        if let Some(loaded) = &restored.loaded_boneyard {
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../web/assets/navigation")
                .join(format!("{}.json", loaded.source_sha256));
            let navigation = std::fs::read_to_string(path)
                .expect("the browser fixture's real stock world has shipped native navigation");
            solomon_assets::navigation::decode_navigation(&navigation, loaded).unwrap();
        }
        let economy = &restored
            .session
            .participants
            .get(&restored.player_id)
            .expect("participant exists")
            .player
            .economy;
        let retained = economy.action_feedback.as_ref();
        assert_eq!(retained.unwrap().sequence, case["retainedSequence"].as_u64().unwrap());
        assert!(NativeSkillBookFeedbackCursor::new(retained).consume(retained).is_none());
        if case["world"] == "hub" {
            let mut document: Value = serde_json::from_str(&imported.document).unwrap();
            let clock = &document["continuation"]["simulation"]["world"]["skorcha"]["gestureTicksRemaining"];
            assert_eq!(clock, 0, "the actual reconstructed actor has not taken its first tick");
            for timer in [0, 1, 29] {
                document["continuation"]["simulation"]["world"]["skorcha"]["gestureTicksRemaining"] =
                    timer.into();
                website_save::restore(&document.to_string()).unwrap();
            }
            for timer in [-1, 30] {
                document["continuation"]["simulation"]["world"]["skorcha"]["gestureTicksRemaining"] =
                    timer.into();
                assert_eq!(
                    website_save::restore(&document.to_string()).unwrap_err(),
                    "game save Skorcha gesture timer is invalid"
                );
            }
        }
        for id in case["bookIds"].as_array().unwrap() {
            let item =
                economy.backpack.iter().find(|item| item.id == id.as_u64().unwrap()).unwrap();
            assert_eq!(item.kind, HubItemKind::SkillBook);
            assert_eq!(item.quantity, 1);
            assert!(item.inventory_slot.is_some());
        }
    }
}
