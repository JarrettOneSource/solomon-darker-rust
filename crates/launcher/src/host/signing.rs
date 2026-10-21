//! Source host identities and signed leaderboard receipts. These use OS entropy and
//! standard cryptographic crates, independently of every gameplay random stream.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::engine::general_purpose::{GeneralPurpose, NO_PAD};
use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;
use subtle::ConstantTimeEq;

use solomon_sim::hall_of_fame::HallOfFameEntry;
use solomon_sim::modding::{LuaObject, LuaValue};
use solomon_sim::social::PartyIdentity;

use super::authority::random_bytes;

const JOIN_CODE_ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
const LEADERBOARD_DOMAIN: &str = "solomon-dark-leaderboard-v1.";
// Node Buffer's decoder accepts nonzero unused trailing bits. Claim syntax separately
// restricts the alphabet/length, so retain that source behavior without relaxing framing.
pub(super) const SOURCE_BASE64URL: GeneralPurpose =
    GeneralPurpose::new(&base64::alphabet::URL_SAFE, NO_PAD.with_decode_allow_trailing_bits(true));

pub(super) fn random_token<const N: usize>() -> Result<String, String> {
    Ok(URL_SAFE_NO_PAD.encode(random_bytes::<N>()?))
}

pub(super) fn random_party_identity() -> Result<PartyIdentity, String> {
    let id = format!("party-{}", random_token::<18>()?);
    let bytes = random_bytes::<8>()?;
    let mut join_code = String::with_capacity(9);
    for (index, byte) in bytes.into_iter().enumerate() {
        if index == 4 {
            join_code.push('-');
        }
        join_code
            .push(char::from(JOIN_CODE_ALPHABET[usize::from(byte) % JOIN_CODE_ALPHABET.len()]));
    }
    Ok(PartyIdentity { id, join_code, listing_id: format!("listing-{}", random_token::<18>()?) })
}

/// Source `credentialsEqual`: length is public; equal-length bytes use a constant-time
/// comparison. This also avoids accidentally treating a Unicode character count as bytes.
pub(super) fn credentials_equal(left: &str, right: &str) -> bool {
    left.len() == right.len() && bool::from(left.as_bytes().ct_eq(right.as_bytes()))
}

pub(super) fn sign(secret: &str, bytes: &[u8]) -> String {
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes())
        .expect("HMAC accepts keys of every byte length");
    mac.update(bytes);
    URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes())
}

pub(super) fn signature_matches(secret: &str, bytes: &[u8], signature: &str) -> bool {
    let Ok(signature) = SOURCE_BASE64URL.decode(signature) else { return false };
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes())
        .expect("HMAC accepts keys of every byte length");
    mac.update(bytes);
    mac.verify_slice(&signature).is_ok()
}

/// `host/game-leaderboard-receipt.ts`: preserve field order and ECMAScript numeric
/// spelling before signing. The existing Website backend verifies this exact envelope.
pub(super) fn leaderboard_receipt(
    secret: &str,
    user_id: u64,
    entry: &HallOfFameEntry,
) -> Result<String, String> {
    if secret.len() < 32 {
        return Err("Leaderboard receipt secret must contain at least 32 bytes".into());
    }
    if !(1..=9_007_199_254_740_991).contains(&user_id) {
        return Err("Leaderboard receipt user id must be a positive safe integer".into());
    }
    let skills = entry
        .highest_skills
        .iter()
        .map(|skill| {
            object([
                ("skillId", LuaValue::Number(skill.skill_id as f64)),
                ("rank", LuaValue::Number(skill.rank as f64)),
            ])
        })
        .collect();
    let payload = object([
        ("version", LuaValue::Number(1.0)),
        ("userId", LuaValue::Number(user_id as f64)),
        ("runId", LuaValue::String(entry.run_id.clone())),
        ("wizardName", LuaValue::String(entry.wizard_name.clone())),
        ("element", LuaValue::String(entry.element.as_str().to_owned())),
        ("discipline", LuaValue::String(entry.discipline.as_str().to_owned())),
        ("headingIndex", LuaValue::Number(entry.heading_index as f64)),
        ("portraitScale", LuaValue::Number(entry.portrait_scale)),
        ("level", LuaValue::Number(entry.level as f64)),
        ("awesomeness", LuaValue::Number(entry.awesomeness as f64)),
        ("elapsedTicks", LuaValue::Number(entry.elapsed_ticks as f64)),
        ("wave", LuaValue::Number(entry.wave as f64)),
        ("monstersKilled", LuaValue::Number(entry.monsters_killed as f64)),
        (
            "awesomestKill",
            entry
                .awesomest_kill
                .as_ref()
                .map_or(LuaValue::Null, |value| LuaValue::String(value.clone())),
        ),
        ("highestSkills", LuaValue::Array(skills)),
        (
            "perksUsed",
            LuaValue::Array(
                entry.perks_used.iter().map(|value| LuaValue::Number(*value as f64)).collect(),
            ),
        ),
        ("completedAtUtc", LuaValue::String(entry.completed_at_utc.clone())),
    ]);
    let json = solomon_assets::json_value::stringify(&payload)?;
    let payload = URL_SAFE_NO_PAD.encode(json.as_bytes());
    let signature = sign(secret, format!("{LEADERBOARD_DOMAIN}{payload}").as_bytes());
    Ok(format!("{payload}.{signature}"))
}

fn object<const N: usize>(fields: [(&str, LuaValue); N]) -> LuaValue {
    let mut object = LuaObject::new();
    for (key, value) in fields {
        object.insert(key.to_owned(), value);
    }
    LuaValue::Object(object)
}

#[cfg(test)]
mod tests {
    use super::*;
    use solomon_sim::hall_of_fame::HallOfFameSkill;
    use solomon_sim::profile::{WizardDiscipline, WizardElement};

    #[test]
    fn leaderboard_receipts_match_the_pinned_website_bytes() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../sim/tests/fixtures/website/host-signing-goldens.json"
        ))
        .unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            let row = &case["entry"];
            let entry = HallOfFameEntry {
                account_username: None,
                awesomeness: row["awesomeness"].as_u64().unwrap(),
                awesomest_kill: row["awesomestKill"].as_str().map(str::to_owned),
                completed_at_utc: row["completedAtUtc"].as_str().unwrap().into(),
                discipline: WizardDiscipline::ALL
                    .into_iter()
                    .find(|value| value.as_str() == row["discipline"].as_str().unwrap())
                    .unwrap(),
                elapsed_ticks: row["elapsedTicks"].as_u64().unwrap(),
                element: WizardElement::ALL
                    .into_iter()
                    .find(|value| value.as_str() == row["element"].as_str().unwrap())
                    .unwrap(),
                heading_index: row["headingIndex"].as_u64().unwrap(),
                highest_skills: row["highestSkills"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|skill| HallOfFameSkill {
                        rank: skill["rank"].as_u64().unwrap(),
                        skill_id: skill["skillId"].as_u64().unwrap(),
                    })
                    .collect(),
                level: row["level"].as_u64().unwrap(),
                monsters_killed: row["monstersKilled"].as_u64().unwrap(),
                perks_used: row["perksUsed"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|value| value.as_u64().unwrap())
                    .collect(),
                portrait_scale: row["portraitScale"].as_f64().unwrap(),
                run_id: row["runId"].as_str().unwrap().into(),
                wave: row["wave"].as_u64().unwrap(),
                wizard_name: row["wizardName"].as_str().unwrap().into(),
            };
            let secret = case["secret"].as_str().unwrap();
            let result =
                leaderboard_receipt(secret, case["userId"].as_i64().unwrap() as u64, &entry);
            if let Some(error) = case["error"].as_str() {
                assert_eq!(result.unwrap_err(), error);
            } else {
                let receipt = result.unwrap();
                assert_eq!(receipt, case["receipt"].as_str().unwrap());
                let (payload, signature) = receipt.split_once('.').unwrap();
                let signed = format!("{LEADERBOARD_DOMAIN}{payload}");
                assert!(signature_matches(secret, signed.as_bytes(), signature));
                assert!(!signature_matches("different key", signed.as_bytes(), signature));
                assert!(!signature_matches(secret, b"changed payload", signature));
            }
        }
        assert!(!credentials_equal("secret", "Secret"));
        assert!(!credentials_equal("é", "e"));
        assert!(credentials_equal("é", "é"));
    }
}
