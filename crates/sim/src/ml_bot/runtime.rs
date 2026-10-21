//! Checkpoint inference from `ml-bot-policy/runtime.ts`; no training or platform runtime.

use super::{
    MlBotPolicyActionIndices, MlBotPolicyActionMaskPlan, MlBotPolicyActionMasks,
    MlBotPolicyChoiceMode, MlBotPolicyTensors, OBSERVATION_WIDTH, OPTION_DESCRIPTOR_WIDTH,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MlBotPolicySelectionMode {
    Argmax,
    Sample,
}

#[derive(Clone, Copy, Debug)]
pub struct MlBotPolicySelectionOptions {
    pub mode: MlBotPolicySelectionMode,
    pub temperature: Option<f64>,
}

impl Default for MlBotPolicySelectionOptions {
    fn default() -> Self {
        Self { mode: MlBotPolicySelectionMode::Argmax, temperature: None }
    }
}

#[derive(Clone, Debug)]
pub struct MlBotPolicyProbabilities {
    pub ability: Vec<f32>,
    pub aim: Vec<f32>,
    pub movement: Vec<f32>,
    pub target: Vec<f32>,
}

#[derive(Clone, Debug)]
pub struct MlBotPolicyHeadResult {
    pub actions: MlBotPolicyActionIndices,
    pub latent: Vec<f32>,
    pub log_probability: f64,
    pub probabilities: MlBotPolicyProbabilities,
    pub value: f64,
}

#[derive(Clone, Debug)]
pub struct MlBotPolicyAutoregressiveResult {
    pub head: MlBotPolicyHeadResult,
    pub masks: MlBotPolicyActionMasks,
}

#[derive(Clone, Debug)]
pub struct MlBotPolicyChoiceResult {
    pub latent: Vec<f32>,
    pub log_probability: f64,
    pub probabilities: Vec<f32>,
    pub selected_option: usize,
    pub value: f64,
}

/// One immutable deployed model. The host can share it across independent bot controllers.
#[derive(Clone, Debug)]
pub struct MlBotPolicyRuntime {
    tensors: MlBotPolicyTensors,
    pub choice_mode: MlBotPolicyChoiceMode,
    pub choice_temperature: f64,
}

impl MlBotPolicyRuntime {
    pub fn new(
        tensors: MlBotPolicyTensors,
        choice_mode: MlBotPolicyChoiceMode,
        choice_temperature: f64,
    ) -> Result<Self, String> {
        tensors.validate()?;
        if !choice_temperature.is_finite() || choice_temperature <= 0.0 {
            return Err("ML bot policy choice temperature must be positive and finite".into());
        }
        Ok(Self { tensors, choice_mode, choice_temperature })
    }

    pub fn infer(
        &self,
        observation: &[f32],
        masks: &MlBotPolicyActionMasks,
        options: MlBotPolicySelectionOptions,
        mut random: Option<&mut dyn FnMut() -> f64>,
    ) -> Result<MlBotPolicyHeadResult, String> {
        validate_observation(observation)?;
        let latent = self.encode(observation)?;
        let movement = head(
            &dense(&latent, &self.tensors.movement_weight, &self.tensors.movement_bias, 9, false)?,
            &masks.movement,
            options,
            &mut random,
        )?;
        let target = head(
            &dense(&latent, &self.tensors.target_weight, &self.tensors.target_bias, 9, false)?,
            &masks.target,
            options,
            &mut random,
        )?;
        let ability = head(
            &dense(&latent, &self.tensors.ability_weight, &self.tensors.ability_bias, 22, false)?,
            &masks.ability,
            options,
            &mut random,
        )?;
        let aim = head(
            &dense(&latent, &self.tensors.aim_weight, &self.tensors.aim_bias, 9, false)?,
            &masks.aim,
            options,
            &mut random,
        )?;
        let value = scalar(&latent, &self.tensors.value_weight, &self.tensors.value_bias)?;
        Ok(head_result(latent, movement, target, ability, aim, value))
    }

    pub fn infer_autoregressive(
        &self,
        observation: &[f32],
        plan: &MlBotPolicyActionMaskPlan,
        options: MlBotPolicySelectionOptions,
        mut random: Option<&mut dyn FnMut() -> f64>,
    ) -> Result<MlBotPolicyAutoregressiveResult, String> {
        validate_observation(observation)?;
        let latent = self.encode(observation)?;
        let movement = head(
            &dense(&latent, &self.tensors.movement_weight, &self.tensors.movement_bias, 9, false)?,
            &plan.movement,
            options,
            &mut random,
        )?;
        let target = head(
            &dense(&latent, &self.tensors.target_weight, &self.tensors.target_bias, 9, false)?,
            &plan.target,
            options,
            &mut random,
        )?;
        let ability_mask = plan
            .ability_by_target
            .get(target.selected)
            .ok_or("ML bot ability mask rows are invalid")?;
        let ability = head(
            &dense(&latent, &self.tensors.ability_weight, &self.tensors.ability_bias, 22, false)?,
            ability_mask,
            options,
            &mut random,
        )?;
        let aim_mask =
            plan.aim_by_ability.get(ability.selected).ok_or("ML bot aim mask rows are invalid")?;
        let aim = head(
            &dense(&latent, &self.tensors.aim_weight, &self.tensors.aim_bias, 9, false)?,
            aim_mask,
            options,
            &mut random,
        )?;
        let masks = MlBotPolicyActionMasks {
            ability: ability_mask.clone(),
            aim: aim_mask.clone(),
            movement: plan.movement.clone(),
            target: plan.target.clone(),
        };
        let value = scalar(&latent, &self.tensors.value_weight, &self.tensors.value_bias)?;
        Ok(MlBotPolicyAutoregressiveResult {
            head: head_result(latent, movement, target, ability, aim, value),
            masks,
        })
    }

    pub fn choose(
        &self,
        observation: &[f32],
        descriptors: &[f32],
        mask: &[u8],
        options: MlBotPolicySelectionOptions,
        mut random: Option<&mut dyn FnMut() -> f64>,
    ) -> Result<MlBotPolicyChoiceResult, String> {
        validate_observation(observation)?;
        if descriptors.is_empty() || !descriptors.len().is_multiple_of(OPTION_DESCRIPTOR_WIDTH) {
            return Err(
                "ML bot policy option descriptors must contain complete nonempty rows".into()
            );
        }
        let option_count = descriptors.len() / OPTION_DESCRIPTOR_WIDTH;
        if mask.len() != option_count {
            return Err("ML bot policy choice mask length is invalid".into());
        }
        for (index, value) in descriptors.iter().enumerate() {
            if !value.is_finite() {
                return Err(format!("ML bot policy option descriptor {index} is not finite"));
            }
        }
        let latent = self.encode(observation)?;
        let mut logits = vec![0.0; option_count];
        for (option, logit) in logits.iter_mut().enumerate() {
            let mut hidden = [0.0_f32; 128];
            for (row, output) in hidden.iter_mut().enumerate() {
                let offset = row * (256 + OPTION_DESCRIPTOR_WIDTH);
                let mut sum = self.tensors.choice_hidden_bias[row];
                for (column, input) in latent.iter().enumerate() {
                    sum = fused(sum, self.tensors.choice_hidden_weight[offset + column], *input);
                }
                for column in 0..OPTION_DESCRIPTOR_WIDTH {
                    sum = fused(
                        sum,
                        self.tensors.choice_hidden_weight[offset + 256 + column],
                        descriptors[option * OPTION_DESCRIPTOR_WIDTH + column],
                    );
                }
                *output = f64::from(sum).tanh() as f32;
            }
            *logit = scalar(
                &hidden,
                &self.tensors.choice_score_weight,
                &self.tensors.choice_score_bias,
            )? as f32;
        }
        let options = MlBotPolicySelectionOptions {
            temperature: Some(options.temperature.unwrap_or(self.choice_temperature)),
            ..options
        };
        let selected = head(&logits, mask, options, &mut random)?;
        Ok(MlBotPolicyChoiceResult {
            value: scalar(
                &latent,
                &self.tensors.choice_value_weight,
                &self.tensors.choice_value_bias,
            )?,
            latent,
            log_probability: selected.log_probability,
            probabilities: selected.probabilities,
            selected_option: selected.selected,
        })
    }

    fn encode(&self, observation: &[f32]) -> Result<Vec<f32>, String> {
        let first = dense(
            observation,
            &self.tensors.trunk_1_weight,
            &self.tensors.trunk_1_bias,
            512,
            true,
        )?;
        dense(&first, &self.tensors.trunk_2_weight, &self.tensors.trunk_2_bias, 256, true)
    }
}

struct HeadResult {
    selected: usize,
    log_probability: f64,
    probabilities: Vec<f32>,
}

fn head_result(
    latent: Vec<f32>,
    movement: HeadResult,
    target: HeadResult,
    ability: HeadResult,
    aim: HeadResult,
    value: f64,
) -> MlBotPolicyHeadResult {
    MlBotPolicyHeadResult {
        actions: MlBotPolicyActionIndices {
            ability: ability.selected,
            aim: aim.selected,
            movement: movement.selected,
            target: target.selected,
        },
        latent,
        log_probability: movement.log_probability
            + target.log_probability
            + ability.log_probability
            + aim.log_probability,
        probabilities: MlBotPolicyProbabilities {
            ability: ability.probabilities,
            aim: aim.probabilities,
            movement: movement.probabilities,
            target: target.probabilities,
        },
        value,
    }
}

fn dense(
    input: &[f32],
    weights: &[f32],
    bias: &[f32],
    width: usize,
    tanh: bool,
) -> Result<Vec<f32>, String> {
    if bias.len() != width || weights.len() != input.len() * width {
        return Err("ML bot policy dense tensor shape is invalid".into());
    }
    let mut output = Vec::with_capacity(width);
    for (row, initial) in bias.iter().enumerate() {
        let mut sum = *initial;
        let offset = row * input.len();
        for (column, value) in input.iter().enumerate() {
            sum = fused(sum, weights[offset + column], *value);
        }
        output.push(if tanh { f64::from(sum).tanh() as f32 } else { sum });
    }
    Ok(output)
}

fn scalar(input: &[f32], weights: &[f32], bias: &[f32]) -> Result<f64, String> {
    if bias.len() != 1 || weights.len() != input.len() {
        return Err("ML bot policy scalar tensor shape is invalid".into());
    }
    let mut sum = bias[0];
    for (value, weight) in input.iter().zip(weights) {
        sum = fused(sum, *weight, *value);
    }
    Ok(f64::from(sum))
}

fn fused(sum: f32, weight: f32, value: f32) -> f32 {
    // The Source helper's name is historical: fround(sum + fround(weight * value))
    // requires two separate Float32 roundings. A hardware FMA would change the policy.
    let product = weight * value;
    sum + product
}

fn head(
    logits: &[f32],
    mask: &[u8],
    options: MlBotPolicySelectionOptions,
    random: &mut Option<&mut dyn FnMut() -> f64>,
) -> Result<HeadResult, String> {
    if mask.len() != logits.len() {
        return Err("ML bot policy mask length is invalid".into());
    }
    let temperature = options.temperature.unwrap_or(1.0);
    if !temperature.is_finite() || temperature <= 0.0 {
        return Err("ML bot policy temperature must be positive and finite".into());
    }
    let mut maximum = f64::NEG_INFINITY;
    let mut valid_count = 0;
    for (logit, allowed) in logits.iter().zip(mask) {
        if *allowed == 1 {
            valid_count += 1;
            maximum = maximum.max(f64::from(*logit) / temperature);
        }
    }
    if valid_count == 0 {
        return Err("ML bot policy mask has no legal action".into());
    }
    let mut probabilities = vec![0.0_f32; logits.len()];
    let mut total = 0.0;
    for (index, logit) in logits.iter().enumerate() {
        if mask[index] == 1 {
            let value = (f64::from(*logit) / temperature - maximum).exp();
            probabilities[index] = value as f32;
            // Source sums the unrounded double exponential, not the Float32 store.
            total += value;
        }
    }
    let mut selected = None;
    let mut best = f64::NEG_INFINITY;
    for (index, probability) in probabilities.iter_mut().enumerate() {
        if mask[index] == 1 {
            *probability = (f64::from(*probability) / total) as f32;
            if options.mode == MlBotPolicySelectionMode::Argmax && f64::from(logits[index]) > best {
                best = f64::from(logits[index]);
                selected = Some(index);
            }
        }
    }
    if options.mode == MlBotPolicySelectionMode::Sample {
        let random =
            random.as_mut().ok_or("ML bot policy sampling requires an injected random source")?;
        let draw = random();
        if !draw.is_finite() || !(0.0..1.0).contains(&draw) {
            return Err("ML bot policy random draw must be within [0, 1)".into());
        }
        let mut cumulative = 0.0;
        for (index, probability) in probabilities.iter().enumerate() {
            cumulative += f64::from(*probability);
            if mask[index] == 1 && draw < cumulative {
                selected = Some(index);
                break;
            }
        }
        if selected.is_none() {
            selected = mask.iter().rposition(|allowed| *allowed == 1);
        }
    }
    let selected = selected.ok_or("ML bot policy failed to select a legal action")?;
    Ok(HeadResult {
        log_probability: f64::from(probabilities[selected]).ln(),
        probabilities,
        selected,
    })
}

fn validate_observation(observation: &[f32]) -> Result<(), String> {
    if observation.len() != OBSERVATION_WIDTH {
        return Err(format!(
            "ML bot policy observation must contain {OBSERVATION_WIDTH} float32 values"
        ));
    }
    for (index, value) in observation.iter().enumerate() {
        if !value.is_finite() {
            return Err(format!("ML bot policy observation {index} is not finite"));
        }
    }
    Ok(())
}
