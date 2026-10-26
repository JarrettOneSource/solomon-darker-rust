//! Dead Spider slide, decal admission and shared lifetime from `native-dead-spider.ts`.
use super::world::EnemyWorld;
use crate::enemies::model::EnemyActorRead;
use crate::float::fround;
use crate::spells::targeting::direction_from_heading;
use crate::{NativeRng, Vec2};

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct SpiderDecal {
    pub entry: usize,
    pub position: Vec2,
    pub rotation_deg: f64,
    pub scale: f64,
    pub alpha: f64,
}
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct DeadSpiderState {
    pub position: Vec2,
    pub heading_deg: f64,
    pub slide_speed: Vec2,
    pub frame: f64,
    pub life: f64,
    pub decal_growth: f64,
    pub decal: Option<SpiderDecal>,
}
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct SpiderRemains {
    pub id: usize,
    pub spawn_tick: u64,
    pub state: DeadSpiderState,
}

#[must_use]
pub fn create_native_dead_spider(position: Vec2, heading: f64) -> DeadSpiderState {
    DeadSpiderState {
        position,
        heading_deg: heading,
        slide_speed: Vec2::new(2.0, 2.0),
        frame: 0.0,
        life: 20.0,
        decal_growth: 0.0,
        decal: None,
    }
}
pub fn step_native_dead_spider(
    mut source: DeadSpiderState,
    rng: &mut NativeRng,
) -> Option<DeadSpiderState> {
    let direction = direction_from_heading(source.heading_deg);
    source.position = Vec2::new(
        fround(source.position.x - direction.x * source.slide_speed.x),
        fround(source.position.y - direction.y * source.slide_speed.y),
    );
    if source.life <= fround(19.2) {
        if source.decal.is_none() {
            let entry = rng.draw_integer(3) as usize;
            let rotation = rng.draw_scaled_float(360.0, false);
            let radius = rng.draw_scaled_float(15.0, false);
            let angle = rng.draw_scaled_float(360.0, false);
            let direction = direction_from_heading(angle);
            source.decal = Some(SpiderDecal {
                entry: [140, 141, 142][entry],
                rotation_deg: rotation,
                position: Vec2::new(
                    fround(source.position.x + direction.x * radius),
                    fround(source.position.y + direction.y * radius),
                ),
                scale: 0.0,
                alpha: 1.0,
            });
            source.decal_growth = fround(0.35);
        }
        source.decal_growth = fround(source.decal_growth + fround(0.01)).min(1.0);
        let decal = source.decal.as_mut().unwrap();
        decal.scale = fround(source.decal_growth * fround(0.9));
        decal.alpha = source.life.min(1.0);
        source.frame = 1.0;
    } else {
        source.frame = fround(source.frame + fround(0.2));
        if source.frame > 2.0 {
            source.frame = fround(source.frame - 2.0);
        }
    }
    source.life = fround(source.life - fround(0.01));
    if source.life <= 0.0 {
        return None;
    }
    source.slide_speed = Vec2::new(
        fround(source.slide_speed.x * fround(0.95)),
        fround(source.slide_speed.y * fround(0.95)),
    );
    Some(source)
}

#[must_use]
pub fn native_dead_spider_entry(source: &DeadSpiderState) -> Option<usize> {
    let mut facing = source.heading_deg / 18.0;
    while facing < 0.0 {
        facing += 10.0;
    }
    while facing > 10.0 {
        facing -= 10.0;
    }
    let index = facing.trunc() as usize + source.frame.trunc() as usize * 10;
    if index < 20 { Some(208 + index) } else { None }
}
impl EnemyWorld {
    pub fn spawn_spider_remains(&mut self, actor: &impl EnemyActorRead, tick: u64) {
        self.spider_remains.push(SpiderRemains {
            id: self.next_death_effect_id,
            spawn_tick: tick,
            state: create_native_dead_spider(actor.position, actor.heading_deg),
        });
        self.next_death_effect_id += 1;
    }
    pub fn step_spider_remains(&mut self) {
        self.spider_remains.retain_mut(|actor| {
            if let Some(state) =
                step_native_dead_spider(actor.state.clone(), &mut self.steering_rng)
            {
                actor.state = state;
                true
            } else {
                false
            }
        });
    }
}
