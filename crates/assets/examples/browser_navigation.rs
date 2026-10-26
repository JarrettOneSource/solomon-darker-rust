//! Read-only navigation queries for browser journeys that drive ordinary keys.
//!
//! The process retains one tracked scene and its prepared meshes. It has no game
//! connection: predictions help choose input, while accepted snapshots own movement.

use std::io::{self, BufRead, Write};
use std::path::PathBuf;

use serde::Deserialize;
use serde_json::{Value, json};
use solomon_assets::{boneyard::BoneyardCatalog, navigation::decode_navigation};
use solomon_sim::Vec2;
use solomon_sim::boneyard::collision::{
    BoneyardCollisionWorld, boneyard_body_collision_source_ids, resolve_boneyard_movement,
};
use solomon_sim::boneyard::encounter::solomon_contact_contains;
use solomon_sim::boneyard::gate::INITIAL_SWAY;
use solomon_sim::boneyard::navigation::RouteRequest;
use solomon_sim::boneyard::{BONEYARD_CAMERA_SCALE, BoneyardBounds, boneyard_camera_origin};
use solomon_sim::float::hypot;
use solomon_sim::hub::PLAYER_CHARACTER_RADIUS;

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Query {
    Route { start: Vec2, end: Vec2, bounds: BoneyardBounds, ignore_end_radius: Option<f64> },
    Direction { start: Vec2, end: Vec2, bounds: BoneyardBounds },
    Contact { solomon: Vec2, player: Vec2 },
    ScreenPoint { position: Vec2, focus: Vec2, bounds: BoneyardBounds, view: Vec2 },
}

fn send(output: &mut impl Write, value: &Value) -> Result<(), Box<dyn std::error::Error>> {
    serde_json::to_writer(&mut *output, value)?;
    writeln!(output)?;
    output.flush()?;
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let source = args.next().ok_or("expected source SHA256")?;
    let geometry = args.next().ok_or("expected geometry SHA256")?;
    let assets = PathBuf::from(args.next().ok_or("expected asset directory")?);
    let catalog = BoneyardCatalog::load_embedded()?;
    let loaded = if catalog.tutorial.source_sha256 == source {
        catalog.materialize_tutorial([0; 16])
    } else {
        let index = catalog
            .templates
            .iter()
            .position(|row| row.source_sha256 == source)
            .ok_or("accepted Boneyard has no tracked template")?;
        let mut seed = [0; 16];
        seed[..4].copy_from_slice(&u32::try_from(index)?.to_be_bytes());
        catalog.materialize(seed)
    };
    if loaded.geometry_sha256 != geometry {
        return Err("accepted Boneyard geometry does not match its tracked template".into());
    }
    let mesh = std::fs::read_to_string(assets.join("navigation").join(format!("{source}.json")))?;
    let navigation = decode_navigation(&mesh, &loaded)?;
    let collision = BoneyardCollisionWorld::from_scene(&loaded.scene);
    let mut output = io::stdout().lock();
    send(
        &mut output,
        &json!({
            "scene": {"spawn": loaded.scene.spawn.position, "bounds": loaded.scene.bounds},
            "bodyRadius": PLAYER_CHARACTER_RADIUS, "gateSway": INITIAL_SWAY,
            "sourceSha256": source, "geometrySha256": geometry,
        }),
    )?;
    for line in io::stdin().lock().lines() {
        let result = match serde_json::from_str::<Query>(&line?)? {
            Query::Route { start, end, bounds, ignore_end_radius } => {
                let ignored = ignore_end_radius.map_or_else(Vec::new, |radius| {
                    boneyard_body_collision_source_ids(end, &collision, radius)
                });
                let ignored: Vec<_> = ignored.iter().map(String::as_str).collect();
                json!(navigation.find_route(
                    RouteRequest {
                        start,
                        end,
                        bounds,
                        body_radius: PLAYER_CHARACTER_RADIUS,
                        clearance: PLAYER_CHARACTER_RADIUS,
                        end_body_radius: Some(0.0),
                        ignored_source_ids: &ignored,
                    },
                    &collision
                ))
            }
            Query::Direction { start, end, bounds } => {
                // Prefer the real key direction that makes collision-resolved progress.
                // Six units approximates movement between browser observations.
                let mut best = Vec2::ZERO;
                let mut remaining = hypot(end.x - start.x, end.y - start.y);
                for (x, y) in [
                    (-1.0, -1.0),
                    (0.0, -1.0),
                    (1.0, -1.0),
                    (-1.0, 0.0),
                    (1.0, 0.0),
                    (-1.0, 1.0),
                    (0.0, 1.0),
                    (1.0, 1.0),
                ] {
                    let length = hypot(x, y);
                    let next = resolve_boneyard_movement(
                        start,
                        Vec2::new(start.x + x / length * 6.0, start.y + y / length * 6.0),
                        bounds,
                        &collision,
                        PLAYER_CHARACTER_RADIUS,
                    );
                    let distance = hypot(end.x - next.x, end.y - next.y);
                    if distance < remaining {
                        best = Vec2::new(x, y);
                        remaining = distance;
                    }
                }
                json!(best)
            }
            Query::Contact { solomon, player } => json!(solomon_contact_contains(solomon, player)),
            Query::ScreenPoint { position, focus, bounds, view } => {
                let origin = boneyard_camera_origin(focus, bounds, view, BONEYARD_CAMERA_SCALE);
                json!(Vec2::new(
                    (position.x - origin.x) * BONEYARD_CAMERA_SCALE,
                    (position.y - origin.y) * BONEYARD_CAMERA_SCALE
                ))
            }
        };
        send(&mut output, &result)?;
    }
    Ok(())
}
