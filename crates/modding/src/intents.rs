//! Atomic game-adapter transactions from `mod-intent-executor.ts`.

use solomon_sim::modding::LuaObject;
use solomon_sim::modding::runtime::{ModIntent, ModStateScope};

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ModIntentExecutionContext {
    pub context: LuaObject,
    pub scope: ModStateScope,
    pub tick: u64,
}

pub trait ModIntentTransaction {
    fn commit(&mut self) -> Result<(), String>;
    fn rollback(&mut self, reason: &str) -> Result<(), String>;
}

/// A transaction borrows the actual game and prepared content for one batch.
/// The executor never retains that borrow or creates another game authority.
pub trait ModIntentAdapter {
    fn prepare<'a>(
        &'a mut self,
        intents: &'a [ModIntent],
        context: &'a ModIntentExecutionContext,
    ) -> Result<Box<dyn ModIntentTransaction + 'a>, String>;
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModIntentExecutionResult {
    pub accepted: bool,
    pub error: Option<String>,
    pub intent_count: usize,
}

pub fn execute_mod_intents(
    adapter: &mut dyn ModIntentAdapter,
    intents: &[ModIntent],
    context: &ModIntentExecutionContext,
) -> ModIntentExecutionResult {
    let reject = |reason: String| ModIntentExecutionResult {
        accepted: false,
        error: Some(reason),
        intent_count: intents.len(),
    };
    if intents.len() > 256 {
        return reject("atomic mod intent batch exceeds 256".into());
    }
    let mut sequences = std::collections::HashSet::new();
    for intent in intents {
        if intent.scope != context.scope {
            return reject(format!("mod intent {} belongs to another scope", intent.sequence));
        }
        if !sequences.insert(intent.sequence) {
            return reject(format!("mod intent sequence is duplicated: {}", intent.sequence));
        }
    }
    let mut transaction = match adapter.prepare(intents, context) {
        Ok(transaction) => transaction,
        Err(error) => return reject(error),
    };
    if let Err(error) = transaction.commit() {
        // Source deliberately keeps the commit error if rollback also fails.
        let _ = transaction.rollback(&error);
        return reject(error);
    }
    ModIntentExecutionResult { accepted: true, error: None, intent_count: intents.len() }
}
