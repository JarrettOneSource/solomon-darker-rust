//! Player/shared stock, purchases and restock clocks from `mod-shop-engine.ts`.

use std::collections::HashSet;
use std::sync::Arc;

use solomon_sim::modding::content::{
    ModShopStockScope, PreparedModContentCatalog, PreparedModShopDefinition,
};
use solomon_sim::modding::runtime::{ModShopCheckpoint, ModShopPurchase, ModShopStockState};

use super::{
    MAXIMUM_SAFE_INTEGER, duration_ticks, validate_player, validate_tick, validate_tick_rate,
};

pub struct ModShopEngine {
    catalog: Arc<PreparedModContentCatalog>,
    ticks_per_second: u32,
    state: ModShopCheckpoint,
}

impl ModShopEngine {
    pub fn new(
        catalog: Arc<PreparedModContentCatalog>,
        ticks_per_second: u32,
    ) -> Result<Self, String> {
        validate_tick_rate(ticks_per_second, "shop")?;
        Ok(Self { catalog, ticks_per_second, state: ModShopCheckpoint::default() })
    }

    pub fn revision(&self) -> u64 {
        self.state.revision
    }
    pub fn checkpoint(&self) -> ModShopCheckpoint {
        self.state.clone()
    }

    pub fn purchase(
        &mut self,
        player_id: &str,
        shop_content_id: &str,
        row: usize,
        gold: u64,
        tick: u64,
    ) -> Result<ModShopPurchase, String> {
        let shop = self.catalog.shop(shop_content_id).ok_or("mod shop purchase is unavailable")?;
        let stock = shop
            .stock
            .get(row)
            .filter(|_| tick <= MAXIMUM_SAFE_INTEGER)
            .ok_or("mod shop purchase is unavailable")?;
        if gold > MAXIMUM_SAFE_INTEGER || gold < stock.price {
            return Err("mod shop purchase has insufficient currency".into());
        }
        let owner = stock_owner(player_id, shop, false)?;
        let index = self.state.stock.iter().position(|current| {
            current.player_id == owner
                && current.shop_content_id == shop_content_id
                && current.row == row as u64
        });
        let current = index.map(|index| &self.state.stock[index]);
        let remaining = current
            .filter(|current| current.restock_tick.is_none_or(|restock| tick < restock))
            .map_or(u64::from(stock.quantity), |current| current.remaining);
        if remaining == 0 {
            return Err("mod shop stock is exhausted".into());
        }
        if index.is_none() && self.state.stock.len() >= 4_096 {
            return Err("mod shop state limit reached".into());
        }
        let state = ModShopStockState {
            player_id: owner,
            remaining: remaining - 1,
            restock_tick: (shop.restock_ms != 0.0)
                .then(|| tick + duration_ticks(shop.restock_ms, self.ticks_per_second)),
            row: row as u64,
            shop_content_id: shop_content_id.into(),
        };
        if let Some(index) = index {
            self.state.stock[index] = state
        } else {
            self.state.stock.push(state)
        }
        self.state.revision += 1;
        Ok(ModShopPurchase {
            item_content_id: stock.item.content_id.clone(),
            price: stock.price,
            quantity: 1,
            row,
            shop_content_id: shop_content_id.into(),
        })
    }

    pub fn remaining(
        &self,
        player_id: &str,
        shop_content_id: &str,
        row: usize,
        tick: u64,
    ) -> Result<u32, String> {
        let Some(shop) = self.catalog.shop(shop_content_id) else { return Ok(0) };
        let Some(stock) = shop.stock.get(row) else { return Ok(0) };
        let owner = stock_owner(player_id, shop, false)?;
        let current = self.state.stock.iter().find(|current| {
            current.player_id == owner
                && current.shop_content_id == shop_content_id
                && current.row == row as u64
        });
        Ok(current
            .filter(|current| current.restock_tick.is_none_or(|restock| restock > tick))
            .map_or(stock.quantity, |current| {
                u32::try_from(current.remaining)
                    .expect("accepted shop definition bounds remaining stock")
            }))
    }

    pub fn project(&self, player_id: Option<&str>) -> Vec<ModShopStockState> {
        self.state
            .stock
            .iter()
            .filter(|row| player_id.is_none_or(|id| row.player_id == id || row.player_id == "*"))
            .cloned()
            .collect()
    }

    pub fn restore(&mut self, checkpoint: ModShopCheckpoint) -> Result<(), String> {
        if checkpoint.revision > MAXIMUM_SAFE_INTEGER || checkpoint.stock.len() > 4_096 {
            return Err("mod shop checkpoint is invalid".into());
        }
        let mut keys = HashSet::new();
        for row in &checkpoint.stock {
            let shop = self.catalog.shop(&row.shop_content_id);
            let stock = shop.and_then(|shop| {
                usize::try_from(row.row).ok().and_then(|index| shop.stock.get(index))
            });
            let owner = shop.map(|shop| stock_owner(&row.player_id, shop, true)).transpose()?;
            if stock.is_none_or(|stock| row.remaining > u64::from(stock.quantity))
                || !keys.insert((owner, &row.shop_content_id, row.row))
                || row.restock_tick.is_some_and(|tick| tick > MAXIMUM_SAFE_INTEGER)
            {
                return Err("mod shop checkpoint contains invalid stock".into());
            }
        }
        self.state = checkpoint;
        Ok(())
    }

    pub fn tick(&mut self, tick: u64) -> Result<bool, String> {
        validate_tick(tick, "shop")?;
        let before = self.state.stock.len();
        self.state.stock.retain(|row| row.restock_tick.is_none_or(|restock| restock > tick));
        let changed = self.state.stock.len() != before;
        if changed {
            self.state.revision += 1
        }
        Ok(changed)
    }
}

fn stock_owner(
    player_id: &str,
    shop: &PreparedModShopDefinition,
    stored: bool,
) -> Result<String, String> {
    validate_player(player_id, "shop")?;
    let requires_player = shop.stock_scope == ModShopStockScope::Player;
    if stored && requires_player != (player_id != "*") {
        return Err("mod shop stock scope does not match its owner".into());
    }
    Ok(if stored || requires_player { player_id.into() } else { "*".into() })
}
