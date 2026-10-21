//! Developer bot policy from Website `core-server/ml-bot-policy`.
//!
//! A checkpoint model is immutable and shared. Per-bot observation and controller memory
//! belongs to the admitted bot, and every intent is applied through the real Session.

mod actions;
mod controller;
pub mod enemies;
pub mod enemy_extensions;
mod entrance;
mod geometry;
pub mod hazards;
pub mod inventory;
pub mod minions;
mod numbers;
mod observer;
pub mod own_effects;
pub mod player_state;
mod runtime;
mod skills;
pub mod spec;
pub mod world_state;

pub use actions::*;
pub use controller::*;
pub use entrance::*;
pub use geometry::*;
pub use observer::*;
pub use runtime::*;
pub use skills::*;

pub const OBSERVATION_WIDTH: usize = 3026;
pub const OPTION_DESCRIPTOR_WIDTH: usize = 138;
pub const MODEL_FORMAT: &str = "solomon-dark-web-bot-policy";
pub const ARCHITECTURE: &str = "mlp-tanh-four-head-v7";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MlBotPolicyChoiceMode {
    Learned,
    Scripted,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MlBotPolicyActionIndices {
    pub ability: usize,
    pub aim: usize,
    pub movement: usize,
    pub target: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MlBotPolicyActionMasks {
    pub ability: Vec<u8>,
    pub aim: Vec<u8>,
    pub movement: Vec<u8>,
    pub target: Vec<u8>,
}

/// The Source worker chooses movement and target first, then the target's ability mask,
/// followed by the selected ability's aim mask. These rows are captured at observation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MlBotPolicyActionMaskPlan {
    pub ability_by_target: Vec<Vec<u8>>,
    pub aim_by_ability: Vec<Vec<u8>>,
    pub movement: Vec<u8>,
    pub target: Vec<u8>,
}

#[derive(Clone, Copy, Debug)]
pub struct MlBotTensorSpec {
    pub name: &'static str,
    pub shape: &'static [usize],
}

pub const TENSOR_SPECS: [MlBotTensorSpec; 20] = [
    MlBotTensorSpec { name: "ability_bias", shape: &[22] },
    MlBotTensorSpec { name: "ability_weight", shape: &[22, 256] },
    MlBotTensorSpec { name: "aim_bias", shape: &[9] },
    MlBotTensorSpec { name: "aim_weight", shape: &[9, 256] },
    MlBotTensorSpec { name: "choice_hidden_bias", shape: &[128] },
    MlBotTensorSpec { name: "choice_hidden_weight", shape: &[128, 394] },
    MlBotTensorSpec { name: "choice_score_bias", shape: &[1] },
    MlBotTensorSpec { name: "choice_score_weight", shape: &[1, 128] },
    MlBotTensorSpec { name: "choice_value_bias", shape: &[1] },
    MlBotTensorSpec { name: "choice_value_weight", shape: &[1, 256] },
    MlBotTensorSpec { name: "movement_bias", shape: &[9] },
    MlBotTensorSpec { name: "movement_weight", shape: &[9, 256] },
    MlBotTensorSpec { name: "target_bias", shape: &[9] },
    MlBotTensorSpec { name: "target_weight", shape: &[9, 256] },
    MlBotTensorSpec { name: "trunk_1_bias", shape: &[512] },
    MlBotTensorSpec { name: "trunk_1_weight", shape: &[512, 3026] },
    MlBotTensorSpec { name: "trunk_2_bias", shape: &[256] },
    MlBotTensorSpec { name: "trunk_2_weight", shape: &[256, 512] },
    MlBotTensorSpec { name: "value_bias", shape: &[1] },
    MlBotTensorSpec { name: "value_weight", shape: &[1, 256] },
];

#[derive(Clone, Debug, Default)]
pub struct MlBotPolicyTensors {
    pub ability_bias: Vec<f32>,
    pub ability_weight: Vec<f32>,
    pub aim_bias: Vec<f32>,
    pub aim_weight: Vec<f32>,
    pub choice_hidden_bias: Vec<f32>,
    pub choice_hidden_weight: Vec<f32>,
    pub choice_score_bias: Vec<f32>,
    pub choice_score_weight: Vec<f32>,
    pub choice_value_bias: Vec<f32>,
    pub choice_value_weight: Vec<f32>,
    pub movement_bias: Vec<f32>,
    pub movement_weight: Vec<f32>,
    pub target_bias: Vec<f32>,
    pub target_weight: Vec<f32>,
    pub trunk_1_bias: Vec<f32>,
    pub trunk_1_weight: Vec<f32>,
    pub trunk_2_bias: Vec<f32>,
    pub trunk_2_weight: Vec<f32>,
    pub value_bias: Vec<f32>,
    pub value_weight: Vec<f32>,
}

impl MlBotPolicyTensors {
    pub fn get(&self, name: &str) -> Option<&[f32]> {
        match name {
            "ability_bias" => Some(&self.ability_bias),
            "ability_weight" => Some(&self.ability_weight),
            "aim_bias" => Some(&self.aim_bias),
            "aim_weight" => Some(&self.aim_weight),
            "choice_hidden_bias" => Some(&self.choice_hidden_bias),
            "choice_hidden_weight" => Some(&self.choice_hidden_weight),
            "choice_score_bias" => Some(&self.choice_score_bias),
            "choice_score_weight" => Some(&self.choice_score_weight),
            "choice_value_bias" => Some(&self.choice_value_bias),
            "choice_value_weight" => Some(&self.choice_value_weight),
            "movement_bias" => Some(&self.movement_bias),
            "movement_weight" => Some(&self.movement_weight),
            "target_bias" => Some(&self.target_bias),
            "target_weight" => Some(&self.target_weight),
            "trunk_1_bias" => Some(&self.trunk_1_bias),
            "trunk_1_weight" => Some(&self.trunk_1_weight),
            "trunk_2_bias" => Some(&self.trunk_2_bias),
            "trunk_2_weight" => Some(&self.trunk_2_weight),
            "value_bias" => Some(&self.value_bias),
            "value_weight" => Some(&self.value_weight),
            _ => None,
        }
    }
    pub fn get_mut(&mut self, name: &str) -> Option<&mut Vec<f32>> {
        match name {
            "ability_bias" => Some(&mut self.ability_bias),
            "ability_weight" => Some(&mut self.ability_weight),
            "aim_bias" => Some(&mut self.aim_bias),
            "aim_weight" => Some(&mut self.aim_weight),
            "choice_hidden_bias" => Some(&mut self.choice_hidden_bias),
            "choice_hidden_weight" => Some(&mut self.choice_hidden_weight),
            "choice_score_bias" => Some(&mut self.choice_score_bias),
            "choice_score_weight" => Some(&mut self.choice_score_weight),
            "choice_value_bias" => Some(&mut self.choice_value_bias),
            "choice_value_weight" => Some(&mut self.choice_value_weight),
            "movement_bias" => Some(&mut self.movement_bias),
            "movement_weight" => Some(&mut self.movement_weight),
            "target_bias" => Some(&mut self.target_bias),
            "target_weight" => Some(&mut self.target_weight),
            "trunk_1_bias" => Some(&mut self.trunk_1_bias),
            "trunk_1_weight" => Some(&mut self.trunk_1_weight),
            "trunk_2_bias" => Some(&mut self.trunk_2_bias),
            "trunk_2_weight" => Some(&mut self.trunk_2_weight),
            "value_bias" => Some(&mut self.value_bias),
            "value_weight" => Some(&mut self.value_weight),
            _ => None,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        for spec in TENSOR_SPECS {
            let tensor =
                self.get(spec.name).expect("the canonical tensor table has one typed field");
            let length: usize = spec.shape.iter().product();
            if tensor.len() != length {
                return Err(format!(
                    "ML bot policy tensor {} must contain {length} float32 values",
                    spec.name
                ));
            }
            for (index, value) in tensor.iter().enumerate() {
                if !value.is_finite() {
                    return Err(format!(
                        "ML bot policy tensor {}[{index}] is not finite",
                        spec.name
                    ));
                }
            }
        }
        Ok(())
    }
}

/// These are Source host intents, applied to the admitted participant by the host.
/// They never own an independent player/session simulation.
#[derive(Clone, Debug, PartialEq)]
pub enum MlBotHostIntent {
    HubAction(crate::economy::HubInventoryAction),
    Input(crate::session::GameSessionInput),
    ScriptedInput(crate::session::GameSessionInput),
    SelectSkill { choice_index: usize, offer_sequence: u64, skill_id: usize },
    ConfirmLoadout(crate::profile::NewGameSelection),
}

pub const ML_BOT_DECISION_INTERVAL_TICKS: u64 = 10;
