//! Actual Source minimap projection across admitted content, surfaces and layers.

#[cfg(feature = "persistence")]
mod replay {
    use serde::Deserialize;
    use solomon_sim::Vec2;
    use solomon_sim::modding::LuaObject;
    use solomon_sim::modding::projection::ModContentProjection;
    use solomon_sim::modding::ui::{
        ModMinimapEnemy, ModMinimapMarkerId, ModMinimapPlayer, ModMinimapSnapshot,
        project_mod_minimap,
    };

    #[derive(Debug, Deserialize, PartialEq)]
    #[serde(tag = "kind", content = "value", rename_all = "kebab-case")]
    enum Id {
        Player(String),
        Enemy(f64),
        ModEnemy(f64),
        Powerup(f64),
    }

    #[derive(Deserialize)]
    struct Player {
        id: String,
        position: [f64; 2],
    }

    #[derive(Deserialize)]
    struct Enemy {
        id: f64,
        position: [f64; 2],
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Snapshot {
        players: Vec<Player>,
        enemies: Vec<Enemy>,
        in_boneyard: bool,
    }

    #[derive(Deserialize)]
    struct Marker {
        id: Id,
        kind: String,
        position: [f64; 2],
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Model {
        accessible_name: String,
        actions: Vec<String>,
        bindings: LuaObject,
        center: [f64; 2],
        content_id: String,
        markers: Vec<Marker>,
        mount: String,
        range: f64,
        size: [f64; 2],
    }

    #[derive(Deserialize)]
    struct Case {
        name: String,
        content: LuaObject,
        runtime: Option<LuaObject>,
        viewer: String,
        snapshot: Snapshot,
        expected: Option<Model>,
    }

    #[derive(Deserialize)]
    struct Fixture {
        cases: Vec<Case>,
    }

    #[test]
    fn source_mod_minimap_projection() {
        let fixture: Fixture =
            serde_json::from_str(include_str!("fixtures/website/mod-minimap-goldens.json"))
                .unwrap();
        for case in fixture.cases {
            let content = ModContentProjection::read(&case.content).unwrap();
            let players: Vec<_> = case
                .snapshot
                .players
                .iter()
                .map(|player| ModMinimapPlayer {
                    id: &player.id,
                    position: Vec2::new(player.position[0], player.position[1]),
                })
                .collect();
            let enemies: Vec<_> = case
                .snapshot
                .enemies
                .iter()
                .map(|enemy| ModMinimapEnemy {
                    id: enemy.id,
                    position: Vec2::new(enemy.position[0], enemy.position[1]),
                })
                .collect();
            let actual = project_mod_minimap(
                &ModMinimapSnapshot {
                    players: &players,
                    enemies: &enemies,
                    in_boneyard: case.snapshot.in_boneyard,
                },
                &content,
                &case.viewer,
                case.runtime.as_ref(),
            );
            assert_eq!(actual.is_some(), case.expected.is_some(), "{}", case.name);
            let (Some(actual), Some(expected)) = (actual, case.expected) else { continue };
            assert_eq!(actual.accessible_name, expected.accessible_name, "{}", case.name);
            assert_eq!(actual.actions, expected.actions, "{}", case.name);
            assert_eq!(
                actual.bindings.cloned().unwrap_or_default(),
                expected.bindings,
                "{}",
                case.name
            );
            assert_eq!([actual.center.x, actual.center.y], expected.center, "{}", case.name);
            assert_eq!(actual.content_id, expected.content_id, "{}", case.name);
            assert_eq!(actual.mount, expected.mount, "{}", case.name);
            assert_eq!(actual.range, expected.range, "{}", case.name);
            assert_eq!([actual.size.x, actual.size.y], expected.size, "{}", case.name);
            assert_eq!(actual.markers.len(), expected.markers.len(), "{}", case.name);
            for (actual, expected) in actual.markers.iter().zip(expected.markers) {
                let id = match actual.id {
                    ModMinimapMarkerId::Player(value) => Id::Player(value.to_owned()),
                    ModMinimapMarkerId::Enemy(value) => Id::Enemy(value),
                    ModMinimapMarkerId::ModEnemy(value) => Id::ModEnemy(value),
                    ModMinimapMarkerId::Powerup(value) => Id::Powerup(value),
                };
                assert_eq!(id, expected.id, "{}", case.name);
                assert_eq!(actual.kind.as_str(), expected.kind, "{}", case.name);
                assert_eq!(
                    [actual.position.x, actual.position.y],
                    expected.position,
                    "{}",
                    case.name
                );
            }
        }
    }
}
