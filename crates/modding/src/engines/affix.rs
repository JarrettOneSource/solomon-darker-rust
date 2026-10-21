//! Content-addressed weighted rolls from `mod-affix-engine.ts`.

use std::sync::Arc;

use solomon_sim::modding::content::PreparedModContentCatalog;
use solomon_sim::modding::runtime::RolledModAffix;

use super::MAXIMUM_SAFE_INTEGER;

pub struct ModAffixEngine {
    catalog: Arc<PreparedModContentCatalog>,
}

impl ModAffixEngine {
    pub fn new(catalog: Arc<PreparedModContentCatalog>) -> Self {
        Self { catalog }
    }

    pub fn roll(
        &self,
        pool_id: &str,
        equipment_type: &str,
        seed: u64,
    ) -> Result<Vec<RolledModAffix>, String> {
        let pool = self
            .catalog
            .affix_pool(pool_id)
            .ok_or_else(|| format!("mod affix pool is unavailable: {pool_id}"))?;
        if seed > MAXIMUM_SAFE_INTEGER {
            return Err("mod affix seed is invalid".into());
        }
        if !pool.applies_to.is_empty() && !pool.applies_to.iter().any(|kind| kind == equipment_type)
        {
            return Ok(Vec::new());
        }
        let mut selected: Vec<RolledModAffix> = Vec::new();
        for roll in 0..pool.rolls {
            let mut candidates = Vec::new();
            for entry in &pool.entries {
                if selected.iter().any(|item| item.content_id == entry.affix.content_id) {
                    continue;
                }
                let affix = self
                    .catalog
                    .affix(&entry.affix.content_id)
                    .expect("prepared pool references an accepted affix");
                if affix.applies_to.is_empty()
                    || affix.applies_to.iter().any(|kind| kind == equipment_type)
                {
                    candidates.push(entry);
                }
            }
            if candidates.is_empty() {
                break;
            }
            let total: f64 = candidates.iter().map(|entry| entry.weight).sum();
            let mut cursor = unit_roll(seed, &pool.rng_domain, roll) * total;
            let mut chosen = *candidates.last().expect("nonempty roll candidates");
            for entry in candidates {
                cursor -= entry.weight;
                if cursor < 0.0 {
                    chosen = entry;
                    break;
                }
            }
            let affix = self
                .catalog
                .affix(&chosen.affix.content_id)
                .expect("prepared pool references an accepted affix");
            selected.push(RolledModAffix {
                content_id: affix.common.content_id.clone(),
                modifiers: affix.modifiers.clone(),
                name: affix.common.name.clone(),
                roll,
            });
        }
        Ok(selected)
    }
}

fn unit_roll(seed: u64, domain: &str, ordinal: u32) -> f64 {
    // JS's bitwise xor intentionally takes only the low 32 seed bits.
    let mut hash = 0x811c_9dc5_u32 ^ seed as u32;
    for byte in format!("{domain}\0{ordinal}").bytes() {
        hash ^= u32::from(byte);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    f64::from(hash) / 4_294_967_296.0
}
