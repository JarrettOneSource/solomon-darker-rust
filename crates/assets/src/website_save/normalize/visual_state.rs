//! Saved shared flashes and rescue Sparkles from protocol/codecs/{screen-flash,native-state}.ts.

use serde_json::Value;

use super::protocol_values::{
    array, boolean, finite, integer, object, positive, string, unit, vector,
};

pub(super) fn screen_flashes(value: &Value, field: &str, tick: u64) -> Result<Value, String> {
    object(value, field, &["epoch", "nextOrder", "writes"])?;
    integer(&value["epoch"], &format!("{field}.epoch"), false)?;
    let next = integer(&value["nextOrder"], &format!("{field}.nextOrder"), true)?;
    let mut previous_order = 0;
    let mut previous_tick = 0;
    for (index, write) in array(&value["writes"], &format!("{field}.writes"))?.iter().enumerate() {
        let key = format!("{field}.writes[{index}]");
        object(write, &key, &["order", "tick", "worldKey", "position", "flash", "onlyIfClear"])?;
        let order = integer(&write["order"], &format!("{key}.order"), true)?;
        let write_tick = integer(&write["tick"], &format!("{key}.tick"), false)?;
        if order <= previous_order
            || order >= next
            || write_tick < previous_tick
            || write_tick > tick
        {
            return Err(format!("{key} must preserve authority order and snapshot time"));
        }
        previous_order = order;
        previous_tick = write_tick;
        world_key(&write["worldKey"], &format!("{key}.worldKey"))?;
        vector(&write["position"], &format!("{key}.position"))?;
        let flash = &write["flash"];
        let key = format!("{key}.flash");
        object(flash, &key, &["alpha", "blue", "decayPerTick", "green", "pointAttenuated", "red"])?;
        let decay = positive(&flash["decayPerTick"], &format!("{key}.decayPerTick"))?;
        if decay > 1.0 {
            return Err(format!("{key}.decayPerTick must be between zero and one"));
        }
        for channel in ["alpha", "blue", "green", "red"] {
            unit(&flash[channel], &format!("{key}.{channel}"))?;
        }
        boolean(&flash["pointAttenuated"], &format!("{key}.pointAttenuated"))?;
        boolean(&write["onlyIfClear"], &format!("{field}.writes[{index}].onlyIfClear"))?;
    }
    Ok(value.clone())
}

pub(super) fn rescue_protection(value: &Value, field: &str) -> Result<Value, String> {
    object(value, field, &["fraction", "nextParticleId", "particles"])?;
    let fraction = unit(&value["fraction"], &format!("{field}.fraction"))?;
    if fraction != solomon_sim::float::fround(fraction) {
        return Err(format!("{field}.fraction must be a native float"));
    }
    let next = integer(&value["nextParticleId"], &format!("{field}.nextParticleId"), false)?;
    let particles = array(&value["particles"], &format!("{field}.particles"))?;
    if particles.len() > 60 {
        return Err(format!("{field}.particles may contain at most 60 entries"));
    }
    let mut previous = 0;
    for (index, particle) in particles.iter().enumerate() {
        let key = format!("{field}.particles[{index}]");
        object(
            particle,
            &key,
            &[
                "alpha",
                "decay",
                "id",
                "position",
                "painterRegistration",
                "rotationDegrees",
                "timer",
                "worldKey",
            ],
        )?;
        let id = integer(&particle["id"], &format!("{key}.id"), false)?;
        let decay = positive(&particle["decay"], &format!("{key}.decay"))?;
        let timer = positive(&particle["timer"], &format!("{key}.timer"))?;
        let rotation = finite(&particle["rotationDegrees"], &format!("{key}.rotationDegrees"))?;
        if rotation < 0.0 {
            return Err(format!("{key}.rotationDegrees must be nonnegative"));
        }
        if id <= previous
            || id >= next
            || !(3.0..=5.0).contains(&decay)
            || timer > 180.0
            || rotation > 360.0
        {
            return Err(format!("{key} has invalid native Sparkle state"));
        }
        previous = id;
        let registration = &particle["painterRegistration"];
        let registration_key = format!("{key}.painterRegistration");
        object(registration, &registration_key, &["managerLane", "registrationOrdinal"])?;
        let lane_key = format!("{registration_key}.managerLane");
        let lane = string(&registration["managerLane"], &lane_key, 16)?;
        if !matches!(lane, "actor" | "transient") {
            return Err(format!("{lane_key} is not supported"));
        }
        if lane != "transient" {
            return Err(format!("{lane_key} must be transient"));
        }
        integer(
            &registration["registrationOrdinal"],
            &format!("{registration_key}.registrationOrdinal"),
            false,
        )?;
        unit(&particle["alpha"], &format!("{key}.alpha"))?;
        vector(&particle["position"], &format!("{key}.position"))?;
        world_key(&particle["worldKey"], &format!("{key}.worldKey"))?;
    }
    if next < 1 {
        return Err(format!("{field}.nextParticleId must be positive"));
    }
    Ok(value.clone())
}

fn world_key(value: &Value, field: &str) -> Result<(), String> {
    string(value, field, 256).map(|_| ())
}
