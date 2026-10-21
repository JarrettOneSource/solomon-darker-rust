//! Source `host/party-recovery-claim.ts`: signed owner continuations bind the account,
//! content, roster and exact normalized Website document. They never trust browser
//! integrity claims to recover a server-ranked lineage.

use std::collections::BTreeSet;
use std::fmt::Write;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use solomon_assets::website_save::{self, SaveIntegrity, SaveWorldKind};
use solomon_sim::combat::PlayerLifeState;
use solomon_sim::game_run::GameRunPhase;
use solomon_sim::modding::LuaValue;
use solomon_sim::profile::WizardElement;
use solomon_sim::social::{GameSessionKind, PartyVisibility};

use super::signing::{SOURCE_BASE64URL, credentials_equal, sign, signature_matches};

pub const MAX_PARTY_RECOVERY_CLAIM_LENGTH: usize = 8_192;
const CLAIM_PREFIX: &str = "sdrpr2";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PartyRecoveryClaim {
    pub content_manifest_sha256: String,
    pub global_score_eligible: bool,
    pub integrity: SaveIntegrity,
    pub leaderboard_user_id: Option<u32>,
    pub party_member_count: usize,
    pub party_leader_player_id: String,
    pub party_roster: Vec<PartyRecoveryRosterMember>,
    #[serde(with = "VisibilityWire")]
    pub party_visibility: PartyVisibility,
    pub player_id: String,
    pub recovery_id: String,
    pub run_id: String,
    #[serde(with = "SessionKindWire")]
    pub session_kind: GameSessionKind,
    pub target_revision: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PartyRecoveryRosterMember {
    pub current_health: f64,
    pub display_name: String,
    #[serde(with = "ElementWire")]
    pub element: WizardElement,
    #[serde(with = "LifeStateWire")]
    pub life_state: PlayerLifeState,
    pub maximum_health: f64,
    pub player_id: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SignedPartyRecoveryClaim {
    #[serde(flatten)]
    claim: PartyRecoveryClaim,
    document_sha256: String,
    version: u8,
}

pub fn create_party_recovery_claim(
    secret: &str,
    claim: &PartyRecoveryClaim,
    unsigned_document: &str,
) -> Result<String, String> {
    validate_secret(secret)?;
    validate_claim(claim)?;
    let normalized = normalized_document(unsigned_document, None)
        .ok_or("party recovery document must have a null claim")?;
    validate_document_bindings(unsigned_document, claim, None)?;
    let signed = SignedPartyRecoveryClaim {
        claim: claim.clone(),
        document_sha256: sha256(&normalized),
        version: 2,
    };
    // Use the existing source JSON.stringify boundary, including unrounded numbers,
    // insertion order and numeric property names in the retained save document.
    // Serializing via the struct preserves its source host property order. Parsing that
    // ordered text as a Lua value then uses the shared ECMAScript number writer.
    let text = serde_json::to_string(&signed).map_err(|error| error.to_string())?;
    let value: LuaValue = serde_json::from_str(&text).map_err(|error| error.to_string())?;
    let payload_json = solomon_assets::json_value::stringify(&value)?;
    let payload = URL_SAFE_NO_PAD.encode(payload_json.as_bytes());
    let signature = sign(secret, payload.as_bytes());
    let token = format!("{CLAIM_PREFIX}.{payload}.{signature}");
    if token.len() > MAX_PARTY_RECOVERY_CLAIM_LENGTH {
        return Err("party recovery claim exceeds its size limit".into());
    }
    Ok(token)
}

pub fn decode_party_recovery_claim(secret: &str, token: &str) -> Option<PartyRecoveryClaim> {
    Some(decode_signed_claim(secret, token)?.claim)
}

pub fn verify_party_recovery_claim(
    secret: &str,
    token: &str,
    document: &str,
) -> Option<PartyRecoveryClaim> {
    let signed = decode_signed_claim(secret, token)?;
    let normalized = normalized_document(document, Some(token))?;
    if !credentials_equal(&signed.document_sha256, &sha256(&normalized)) {
        return None;
    }
    validate_document_bindings(document, &signed.claim, Some(token)).ok()?;
    Some(signed.claim)
}

pub fn party_recovery_claim_token(token: &str) -> bool {
    if token.len() > MAX_PARTY_RECOVERY_CLAIM_LENGTH {
        return false;
    }
    let mut parts = token.split('.');
    let (Some(prefix), Some(payload), Some(signature), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return false;
    };
    prefix == CLAIM_PREFIX
        && !payload.is_empty()
        && base64url(payload)
        && signature.len() == 43
        && base64url(signature)
}

fn decode_signed_claim(secret: &str, token: &str) -> Option<SignedPartyRecoveryClaim> {
    validate_secret(secret).ok()?;
    if !party_recovery_claim_token(token) {
        return None;
    }
    let mut parts = token.split('.');
    parts.next()?;
    let payload = parts.next()?;
    let signature = parts.next()?;
    if !signature_matches(secret, payload.as_bytes(), signature) {
        return None;
    }
    let bytes = SOURCE_BASE64URL.decode(payload).ok()?;
    let value: serde_json::Value = serde_json::from_str(&String::from_utf8_lossy(&bytes)).ok()?;
    let object = value.as_object()?;
    const KEYS: [&str; 15] = [
        "contentManifestSha256",
        "documentSha256",
        "globalScoreEligible",
        "integrity",
        "leaderboardUserId",
        "partyLeaderPlayerId",
        "partyMemberCount",
        "partyRoster",
        "partyVisibility",
        "playerId",
        "recoveryId",
        "runId",
        "sessionKind",
        "targetRevision",
        "version",
    ];
    let expected = &KEYS;
    if object.len() != expected.len() || expected.iter().any(|key| !object.contains_key(*key)) {
        return None;
    }
    let signed: SignedPartyRecoveryClaim = serde_json::from_value(value).ok()?;
    if signed.version != 2 || !hex(&signed.document_sha256, 64) {
        return None;
    }
    validate_claim(&signed.claim).ok()?;
    Some(signed)
}

fn validate_claim(claim: &PartyRecoveryClaim) -> Result<(), String> {
    if !hex(&claim.content_manifest_sha256, 64) {
        return Err("party recovery content digest is invalid".into());
    }
    if claim.leaderboard_user_id.is_some_and(|id| id == 0 || id > i32::MAX as u32) {
        return Err("party recovery leaderboard user is invalid".into());
    }
    if !(1..=16).contains(&claim.party_member_count) {
        return Err("party recovery member count is invalid".into());
    }
    if !identity(&claim.party_leader_player_id) {
        return Err("party recovery leader is invalid".into());
    }
    if claim.party_roster.len() != claim.party_member_count {
        return Err("party recovery roster size is invalid".into());
    }
    let mut ids = BTreeSet::new();
    for member in &claim.party_roster {
        if !identity(&member.player_id) {
            return Err("party recovery roster player is invalid".into());
        }
        if member.display_name.is_empty() || member.display_name.encode_utf16().count() > 64 {
            return Err("party recovery roster name is invalid".into());
        }
        if !member.current_health.is_finite()
            || !member.maximum_health.is_finite()
            || member.maximum_health <= 0.0
        {
            return Err("party recovery roster health is invalid".into());
        }
        if !ids.insert(member.player_id.as_str()) {
            return Err("party recovery roster contains a duplicate".into());
        }
    }
    if !ids.contains(claim.party_leader_player_id.as_str())
        || !ids.contains(claim.player_id.as_str())
    {
        return Err("party recovery roster is missing its leader or claimant".into());
    }
    if !identity(&claim.player_id) {
        return Err("party recovery player id is invalid".into());
    }
    if claim.recovery_id.len() != 43 || !base64url(&claim.recovery_id) {
        return Err("party recovery recovery id is invalid".into());
    }
    if !identity(&claim.run_id) {
        return Err("party recovery run id is invalid".into());
    }
    if claim.target_revision.as_ref().is_some_and(|revision| !hex(revision, 40)) {
        return Err("party recovery target revision is invalid".into());
    }
    if claim.session_kind == GameSessionKind::GlobalHub
        && claim.integrity != SaveIntegrity::GlobalClean
    {
        return Err("party recovery global session integrity is invalid".into());
    }
    Ok(())
}

fn validate_document_bindings(
    document: &str,
    claim: &PartyRecoveryClaim,
    token: Option<&str>,
) -> Result<(), String> {
    let parsed = website_save::parse(document)?;
    let continuation =
        parsed.continuation.as_ref().ok_or("party recovery document bindings are invalid")?;
    let summary = &continuation.summary;
    if !(12..=website_save::SCHEMA_VERSION).contains(&parsed.source_schema_version)
        || summary.party_rejoin_token.as_deref() != token
        || summary.player_id != claim.player_id
        || summary.world_kind != SaveWorldKind::Boneyard
        || summary.phase != GameRunPhase::Active
        || !summary.active_run
        || parsed.metadata.integrity != claim.integrity
    {
        return Err("party recovery document bindings are invalid".into());
    }
    let restored = website_save::restore(document)?;
    if restored.player_id != claim.player_id
        || restored.loaded_boneyard.as_ref().is_none_or(|loaded| loaded.run_id != claim.run_id)
        || restored
            .session
            .boneyard
            .as_ref()
            .is_none_or(|world| world.loaded.run_id != claim.run_id)
        || restored.session.run.run_id.as_deref() != Some(claim.run_id.as_str())
        || restored.session.run.phase != GameRunPhase::Active
    {
        return Err("party recovery run bindings are invalid".into());
    }
    Ok(())
}

fn normalized_document(document: &str, expected_token: Option<&str>) -> Option<String> {
    let mut value: LuaValue = serde_json::from_str(document).ok()?;
    let LuaValue::Object(root) = &mut value else { return None };
    let LuaValue::Object(continuation) = root.get_mut("continuation")? else { return None };
    let LuaValue::Object(summary) = continuation.get_mut("summary")? else { return None };
    let token = summary.get_mut("partyRejoinToken")?;
    match (&*token, expected_token) {
        (LuaValue::Null, None) => {}
        (LuaValue::String(actual), Some(expected)) if actual == expected => {}
        _ => return None,
    }
    *token = LuaValue::Null;
    solomon_assets::json_value::stringify(&value).ok()
}

fn validate_secret(secret: &str) -> Result<(), String> {
    if secret.len() < 32 {
        return Err("party recovery secret must contain at least 32 bytes".into());
    }
    Ok(())
}
fn sha256(value: &str) -> String {
    let digest = Sha256::digest(value.as_bytes());
    let mut output = String::with_capacity(digest.len() * 2);
    for byte in digest {
        write!(&mut output, "{byte:02x}").expect("writing into String cannot fail");
    }
    output
}
fn identity(value: &str) -> bool {
    !value.is_empty() && value.encode_utf16().count() <= 128
}
fn base64url(value: &str) -> bool {
    value.bytes().all(|value| value.is_ascii_alphanumeric() || matches!(value, b'-' | b'_'))
}
fn hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value.bytes().all(|value| value.is_ascii_digit() || (b'a'..=b'f').contains(&value))
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "GameSessionKind", rename_all = "kebab-case")]
enum SessionKindWire {
    GlobalHub,
    PrivateCollege,
    Standalone,
}
#[derive(Serialize, Deserialize)]
#[serde(remote = "PartyVisibility", rename_all = "kebab-case")]
enum VisibilityWire {
    Public,
    InviteOnly,
    Private,
}
#[derive(Serialize, Deserialize)]
#[serde(remote = "WizardElement", rename_all = "lowercase")]
enum ElementWire {
    Air,
    Earth,
    Ether,
    Fire,
    Water,
}
#[derive(Serialize, Deserialize)]
#[serde(remote = "PlayerLifeState", rename_all = "kebab-case")]
enum LifeStateWire {
    Alive,
    LethalPending,
    Dying,
    Spectating,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn checked_document(unsigned: &str, token: &str, mode: &str) -> String {
        if mode == "unsigned" {
            return unsigned.to_owned();
        }
        let mut value: LuaValue = serde_json::from_str(unsigned).unwrap();
        let LuaValue::Object(root) = &mut value else { panic!("source fixture root") };
        {
            let Some(LuaValue::Object(continuation)) = root.get_mut("continuation") else {
                panic!("source continuation")
            };
            let Some(LuaValue::Object(summary)) = continuation.get_mut("summary") else {
                panic!("source summary")
            };
            summary.insert("partyRejoinToken".into(), LuaValue::String(token.into()));
            if mode == "changed-owner" {
                summary.insert("playerId".into(), LuaValue::String("other".into()));
            }
        }
        if mode == "changed-gold" {
            let Some(LuaValue::Object(profile)) = root.get_mut("profile") else {
                panic!("source profile")
            };
            let Some(LuaValue::Object(economy)) = profile.get_mut("economy") else {
                panic!("source economy")
            };
            let Some(LuaValue::Number(gold)) = economy.get_mut("gold") else {
                panic!("source gold")
            };
            *gold += 1.0;
        }
        solomon_assets::json_value::stringify(&value).unwrap()
    }

    #[test]
    fn recovery_claims_and_document_bindings_match_the_pinned_host() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../sim/tests/fixtures/website/party-recovery-goldens.json"
        ))
        .unwrap();
        for row in fixture["creates"].as_array().unwrap() {
            let claim: PartyRecoveryClaim = serde_json::from_value(row["claim"].clone()).unwrap();
            let actual = create_party_recovery_claim(
                row["secret"].as_str().unwrap(),
                &claim,
                fixture["unsignedDocument"].as_str().unwrap(),
            );
            if let Some(error) = row["error"].as_str() {
                assert_eq!(actual.unwrap_err(), error, "{}", row["name"]);
            } else {
                assert_eq!(actual.unwrap(), row["token"].as_str().unwrap(), "{}", row["name"]);
            }
        }
        for row in fixture["checks"].as_array().unwrap() {
            let secret = row["secret"].as_str().unwrap();
            let token = row["token"].as_str().unwrap();
            let document = checked_document(
                fixture["unsignedDocument"].as_str().unwrap(),
                token,
                row["documentMode"].as_str().unwrap(),
            );
            let expected = |key: &str| -> Option<PartyRecoveryClaim> {
                serde_json::from_value(row[key].clone()).unwrap()
            };
            assert_eq!(
                party_recovery_claim_token(token),
                row["syntax"].as_bool().unwrap(),
                "{} syntax",
                row["name"]
            );
            assert_eq!(
                decode_party_recovery_claim(secret, token),
                expected("decoded"),
                "{} decode",
                row["name"]
            );
            assert_eq!(
                verify_party_recovery_claim(secret, token, &document),
                expected("verified"),
                "{} verify",
                row["name"]
            );
        }
    }
}
