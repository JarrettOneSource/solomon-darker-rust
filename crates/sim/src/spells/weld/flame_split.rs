//! Current Source `renderer/primary-spell-weld-view.ts`: Flame's padded owner
//! determines band count; the Region view only moves the first band and masks it.

use super::NativeFlameLashBounds;
use crate::float::fround;

#[derive(Clone, Debug, PartialEq)]
pub struct NativeFlameLashSplitBand {
    pub id: String,
    pub clip: NativeFlameLashBounds,
    pub painter_y: f64,
}

pub fn build_native_flame_lash_split_bands(
    owner_id: &str,
    bounds: NativeFlameLashBounds,
    enhanced_effects: bool,
    region_view: Option<NativeFlameLashBounds>,
) -> Result<Vec<NativeFlameLashSplitBand>, String> {
    if owner_id.is_empty()
        || !valid_bounds(bounds)
        || region_view.is_some_and(|region| !valid_bounds(region))
    {
        return Err("Flame split requires finite nonnegative bounds and Region view".into());
    }
    let height = if enhanced_effects { 25.0 } else { 50.0 };
    let mut y = fround(region_view.map_or(bounds.y, |region| bounds.y.max(region.y)));
    let mut bands = Vec::new();
    let mut progress = bounds.y;
    while progress < bounds.y + bounds.height {
        let next = fround(progress + height);
        if next <= progress || bands.len() >= 32_768 {
            return Err("Flame split exceeds its finite band progression budget".into());
        }
        bands.push(NativeFlameLashSplitBand {
            id: format!("{owner_id}:band-{}", bands.len()),
            clip: NativeFlameLashBounds { x: 0.0, y, width: 10_000.0, height },
            painter_y: fround(y + height),
        });
        y = fround(y + height);
        progress = next;
    }
    Ok(bands)
}

/// The actual draw mask can be empty while its admitted insertion still exists.
#[must_use]
pub fn native_flame_region_clipped_band(
    band: NativeFlameLashBounds,
    region: Option<NativeFlameLashBounds>,
) -> NativeFlameLashBounds {
    let Some(region) = region else {
        return band;
    };
    let x = band.x.max(region.x);
    let y = band.y.max(region.y);
    NativeFlameLashBounds {
        x,
        y,
        width: ((band.x + band.width).min(region.x + region.width) - x).max(0.0),
        height: ((band.y + band.height).min(region.y + region.height) - y).max(0.0),
    }
}

fn valid_bounds(bounds: NativeFlameLashBounds) -> bool {
    bounds.x.is_finite()
        && bounds.y.is_finite()
        && bounds.width.is_finite()
        && bounds.height.is_finite()
        && bounds.width >= 0.0
        && bounds.height >= 0.0
}
