//! Horizontal bands that split a Flame Lash for painter sorting. Flame's padded owner
//! determines band count; the Region view only moves the first band and masks it.

use super::FlameLashBounds;
use crate::float::fround;
use crate::painter::zanim_split_band_height;

/// One horizontal band of a split Flame Lash: its clip rectangle and painter Y.
#[derive(Clone, Debug, PartialEq)]
pub struct FlameLashSplitBand {
    pub id: String,
    pub clip: FlameLashBounds,
    pub painter_y: f64,
}

pub fn build_flame_lash_split_bands(
    owner_id: &str,
    bounds: FlameLashBounds,
    enhanced_effects: bool,
    region_view: Option<FlameLashBounds>,
) -> Result<Vec<FlameLashSplitBand>, String> {
    if owner_id.is_empty()
        || !valid_bounds(bounds)
        || region_view.is_some_and(|region| !valid_bounds(region))
    {
        return Err("Flame split requires finite nonnegative bounds and Region view".into());
    }
    let height = zanim_split_band_height(enhanced_effects);
    let mut y = fround(region_view.map_or(bounds.y, |region| bounds.y.max(region.y)));
    let mut bands = Vec::new();
    let mut progress = bounds.y;
    while progress < bounds.y + bounds.height {
        let next = fround(progress + height);
        if next <= progress || bands.len() >= 32_768 {
            return Err("Flame split exceeds its finite band progression budget".into());
        }
        bands.push(FlameLashSplitBand {
            id: format!("{owner_id}:band-{}", bands.len()),
            clip: FlameLashBounds { x: 0.0, y, width: 10_000.0, height },
            painter_y: fround(y + height),
        });
        y = fround(y + height);
        progress = next;
    }
    Ok(bands)
}

/// The actual draw mask can be empty while its admitted insertion still exists.
#[must_use]
pub fn flame_region_clipped_band(
    band: FlameLashBounds,
    region: Option<FlameLashBounds>,
) -> FlameLashBounds {
    let Some(region) = region else {
        return band;
    };
    let x = band.x.max(region.x);
    let y = band.y.max(region.y);
    FlameLashBounds {
        x,
        y,
        width: ((band.x + band.width).min(region.x + region.width) - x).max(0.0),
        height: ((band.y + band.height).min(region.y + region.height) - y).max(0.0),
    }
}

fn valid_bounds(bounds: FlameLashBounds) -> bool {
    bounds.x.is_finite()
        && bounds.y.is_finite()
        && bounds.width.is_finite()
        && bounds.height.is_finite()
        && bounds.width >= 0.0
        && bounds.height >= 0.0
}
