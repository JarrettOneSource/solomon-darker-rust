//! Historical growing effects and settled Bouncers from `game-save-document.ts`.

use serde_json::{Value, json};
use solomon_sim::float::fround;

use super::super::super::contract::{MAX_SAFE_INTEGER, record};
use super::super::{array, finite, integer_within};

pub(super) fn normalize(value: &Value, version: u64) -> Result<Value, String> {
    let mut effects = Vec::new();
    for value in array(value, "saved enemy death effects")? {
        if version >= 48 {
            effects.push(value.clone());
            continue;
        }
        record(value, "saved enemy death effect")?;
        let Some(mut effect) =
            (if version < 39 { growing(value, version)? } else { Some(value.clone()) })
        else {
            continue;
        };
        if version < 33 {
            effect["scaleY"] = effect["scale"].clone();
        }
        if version < 34 && effect["kind"] == "unbind" {
            effect["presentationOwner"] = json!("late-world-overlay");
            effect["painterRegistration"] = Value::Null;
        }
        if version < 36 && effect["kind"] == "fade-perspective-clipped" {
            effect["presentationOwner"] = json!("background");
            effect["painterRegistration"] = Value::Null;
        }
        if matches!(
            effect["kind"].as_str(),
            Some("bouncer" | "smoky-bouncer" | "black-smoky-bouncer")
        ) && effect["height"] == 0
            && effect["verticalVelocity"] == 0
            && effect["angularVelocityDeg"] == 0
        {
            let velocity = record(&effect["velocity"], "saved settled Bouncer velocity")?;
            if velocity.get("x") == Some(&json!(0)) && velocity.get("y") == Some(&json!(0)) {
                effect["bounceVelocity"] = json!(0);
            }
        }
        effects.push(effect);
    }
    Ok(Value::Array(effects))
}

fn growing(source: &Value, version: u64) -> Result<Option<Value>, String> {
    if !matches!(source["kind"].as_str(), Some("fade-scale" | "fade-scale-perspective")) {
        return Ok(Some(source.clone()));
    }
    let scale = finite(&source["scale"], "saved scale-fade X scale")?;
    let scale_y =
        finite(&source[if version < 33 { "scale" } else { "scaleY" }], "saved scale-fade Y scale")?;
    let mut effect = source.clone();
    effect["scale"] = json!(fround(scale));
    effect["scaleY"] = json!(fround(scale_y));
    let ring =
        matches!(source["role"].as_str(), Some("ultra-banish-ring" | "discorporeal-death-ring"));
    let wraith = source["role"] == "wraith-dissolve-core";
    let tragic = source["role"] == "tragic-circle-contact";
    if !ring && !wraith && !tragic {
        return Ok(Some(effect));
    }
    let last_tick = finite(&source["lastStepTick"], "saved scale-fade last tick")?;
    if last_tick.fract() != 0.0 || last_tick < -1.0 || last_tick > MAX_SAFE_INTEGER as f64 {
        return Err("saved scale-fade last tick is invalid".into());
    }
    let spawn =
        integer_within(&source["spawnTick"], "saved scale-fade birth", 0, MAX_SAFE_INTEGER)?;
    let updates = (last_tick - spawn as f64 + f64::from(ring)).max(0.0) as u64;
    let mut multiplier = fround(finite(&source["scaleMultiplier"], "saved scale-fade multiplier")?);
    let mut opacity = if ring { 3.0 } else { 2.0 };
    let mut initial_scale = if ring { 2.0 } else { 1.0 };
    let loss = if ring {
        fround(fround(0.1) * 0.25)
    } else if wraith {
        fround(0.1)
    } else {
        fround(fround(0.1) * 0.5)
    };
    if source["role"] == "ultra-banish-ring"
        && source["alphaLossPerTick"].as_f64() == Some(fround(0.005))
    {
        let index = ((multiplier - 1.045) / fround(0.025) * 100000.0 + 0.5).floor();
        let magnitude = fround(fround(index / 100000.0) * fround(0.025));
        if !(0.0..=100000.0).contains(&index) || fround(1.045 + magnitude) != multiplier {
            return Err("saved UltraBanish multiplier is invalid".into());
        }
        multiplier = fround(fround(1.045) + magnitude);
    }
    if wraith {
        record(&source["position"], "saved Wraith core position")?;
        effect["position"] = json!({
            "x":fround(finite(&source["position"]["x"], "saved Wraith core X")? - 1.0),
            "y":fround(finite(&source["position"]["y"], "saved Wraith core Y")?)
        });
        multiplier = fround(1.02);
    }
    if tragic {
        if updates >= 16 {
            return Ok(None);
        }
        let previous_loss = finite(&source["alphaLossPerTick"], "saved Tragic Circle loss")?;
        opacity = finite(&source["opacityTimer"], "saved Tragic Circle opacity")?;
        let current_opacity = opacity;
        for _ in 0..updates {
            opacity = fround(opacity + previous_loss);
        }
        let mut check = opacity;
        for _ in 0..updates {
            check = fround(check - previous_loss);
        }
        if !(0.5..=0.75).contains(&opacity) || check != current_opacity {
            return Err("saved Tragic Circle opacity is invalid".into());
        }
        let previous_scale = fround(scale / multiplier.powf(updates as f64));
        let index = ((previous_scale - 1.0).abs() / fround(0.65) * 100000.0 + 0.5).floor();
        let magnitude = fround(index / 100000.0);
        let signed = if previous_scale < 1.0 { -magnitude } else { magnitude };
        if !(0.0..=100000.0).contains(&index)
            || fround(1.0 + signed * fround(0.65)) != previous_scale
        {
            return Err("saved Tragic Circle scale is invalid".into());
        }
        initial_scale = fround(1.0 + magnitude * fround(0.65));
        multiplier = fround(1.1);
        effect["blendMode"] = json!("add");
    }
    let mut grown_scale = initial_scale;
    for _ in 0..updates {
        opacity = fround(opacity - loss);
        if opacity <= 0.0 {
            return Ok(None);
        }
        grown_scale = fround(grown_scale * multiplier);
    }
    effect["alphaLossPerTick"] = json!(loss);
    effect["opacityTimer"] = json!(opacity);
    effect["alpha"] = json!(
        opacity.min(1.0) * finite(&source["alphaMultiplier"], "saved scale-fade alpha multiplier")?
    );
    effect["scale"] = json!(grown_scale);
    effect["scaleY"] = json!(grown_scale);
    effect["scaleMultiplier"] = json!(multiplier);
    Ok(Some(effect))
}
