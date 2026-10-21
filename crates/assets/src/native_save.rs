//! Native `.cfg`/`.sav` and launcher archive interchange from Website `save/`.
//! Unknown native payloads remain in their original tree so exporting does not lose them.

mod archive;
mod bridge;
mod codec;
mod portable;
mod runtime;
mod template;

pub use archive::{
    NativeArchiveFile, NativeSaveArchive, WEB_GAME_SAVE_SUPPORT_ARCHIVE_PATH,
    create_native_save_archive, create_stored_zip, native_archive_crc32, read_native_save_archive,
    read_zip,
};

pub use bridge::{
    NATIVE_FIRST_MIX_COUNT, NATIVE_HAGATHA_OWNERSHIP_COUNT, NATIVE_SOURCE_ATTACHMENT_MAX_BYTES,
    NATIVE_WIZARD_SKILL_ROW_COUNT, NativeDarkdataProfile, NativeGameBoastState, NativeProfilePatch,
    NativeProgressionRow, NativeWizardPatch, NativeWizardProgression, RETAIL_SOLOMON_DARK_SHA256,
    decode_native_darkdata_profile, decode_native_gamestate_boast, decode_native_gamestate_wizard,
    patch_native_darkdata, patch_native_gamestate,
};

pub use codec::{
    NATIVE_DARKDATA_KEY, NativeChunkNode, NativeNamedBuffer, NativeSyncBuffer,
    decode_native_darkdata, encode_native_darkdata, encode_native_sync_buffer,
    parse_native_sync_buffer,
};

fn sha256(bytes: &[u8]) -> String {
    use std::fmt::Write;

    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity(64);
    for byte in &digest {
        write!(&mut output, "{byte:02x}").expect("writing a digest to a String cannot fail");
    }
    output
}

fn sha256_is_valid(value: &str) -> bool {
    value.len() == 64
        && value.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn safe_integer(value: &serde_json::Value) -> Option<i64> {
    let value = value.as_f64()?;
    if !value.is_finite() || value.fract() != 0.0 || value.abs() > 9_007_199_254_740_991.0 {
        return None;
    }
    Some(value as i64)
}

fn safe_unsigned(value: &serde_json::Value) -> Option<u64> {
    u64::try_from(safe_integer(value)?).ok()
}

pub use portable::{
    MAX_PORTABLE_GAME_PROFILE_BYTES, PORTABLE_GAME_PROFILE_FORMAT, PORTABLE_GAME_PROFILE_VERSION,
    PortableGameProfile, PortableGameProfileState, PortableGameWizardState,
    PortableNativeStorageSummary, create_native_game_save_source,
    create_portable_game_profile_from_native, encode_portable_game_profile, native_source_bytes,
    parse_portable_game_profile, validate_native_game_save_source,
};
pub use template::{decode_native_hub_template, load_embedded_native_hub_template};

/// Match atob before the source's separate canonical-base64 integrity check.
fn decode_base64(value: &str, claim: &str) -> Result<Vec<u8>, String> {
    use base64::Engine;
    use base64::alphabet;
    use base64::engine::{DecodePaddingMode, GeneralPurpose, GeneralPurposeConfig};

    let mut compact: Vec<u8> =
        value.bytes().filter(|byte| !matches!(byte, b'\t' | b'\n' | 0x0c | b'\r' | b' ')).collect();
    if compact.len().is_multiple_of(4) {
        if compact.last() == Some(&b'=') {
            compact.pop();
        }
        if compact.last() == Some(&b'=') {
            compact.pop();
        }
    }
    let engine = GeneralPurpose::new(
        &alphabet::STANDARD,
        GeneralPurposeConfig::new()
            .with_decode_padding_mode(DecodePaddingMode::RequireNone)
            .with_decode_allow_trailing_bits(true),
    );
    engine.decode(&compact).map_err(|_| format!("{claim} is not valid base64"))
}

pub use runtime::{
    GameSaveImportPreview, GameSaveImportSource, GameSaveInputFile, PortableExportResult,
    PortableImportResult, create_portable_game_profile_from_web_save,
    create_web_game_save_from_portable_profile, export_web_game_save_to_native_archive,
    read_game_save_file_selection,
};
