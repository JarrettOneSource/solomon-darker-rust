//! The summary leaves the save server, so an error message may name only the game's own
//! fields, types and rules. Most codec messages already do. The ones that also repeat what a
//! document said are listed here with their sources, and their values are replaced with a
//! placeholder naming what was there:
//!
//! - serde names the value it received (`invalid type: string "Bob", expected u64`) and
//!   the unknown variant or field (`` unknown variant `x`, expected one of ... ``);
//! - `{field} has unexpected field {key}` (`website_save::contract::only_keys`,
//!   `normalize::reject_unexpected_keys`, the profile economy) and `{field}.{key} is not
//!   allowed` (`normalize::protocol_values::object`) repeat a document key;
//! - `{type} has unknown value {tag}` (`website_save::value`) repeats an enum tag;
//! - `legacy Goodie {eid} is absent from its Boneyard scene` repeats a scenery identity, and
//!   `game save secondary player {id} must be an object` a player identity;
//! - `{scene}: Boneyard bounds must have positive size` and `{scene}: Road {eid} has an
//!   invalid link mask` (`boneyard::decode_scene`) repeat a Boneyard's name, which a
//!   community author chose;
//! - `Cannot load native navigation {path}: {error}` (`HostContent::load_navigation`)
//!   repeats the Boneyard source hash in a server path.
//!
//! Every number becomes `#`, since an index or count can be a value. A message that is
//! still longer than any codec message, or that still contains a name, identity or other
//! long string from the document, is withheld whole, so a template missing from this list
//! cannot leak a player's text.

use std::collections::BTreeSet;

use serde_json::Value;

/// What a message becomes when it still repeats text from its document after redaction.
pub const WITHHELD: &str = "message withheld: it repeated text from the document";

/// The placeholders redaction writes, ignored when looking for document text.
const PLACEHOLDERS: [&str; 6] = ["<value>", "<field>", "<id>", "<scene>", "<file>", "#"];

/// Longer messages are withheld: codec messages, including serde's lists of expected
/// variants, are much shorter, and no collected value can hide in a shorter message.
const MAX_MESSAGE_CHARS: usize = 2048;

/// Fields whose string values name a player, party, run, Boneyard or mod, or hold text a
/// player or author chose.
const IDENTIFYING_FIELDS: &[&str] = &[
    "displayName",
    "eid",
    "id",
    "itemName",
    "modId",
    "modName",
    "name",
    "ownerId",
    "partyRejoinToken",
    "path",
    "playerId",
    "runId",
    "runName",
    "seed",
    "token",
    "wizardName",
];

/// String values at least this long count as document text wherever they occur: they are
/// identities, hashes, tokens or sentences rather than the short tags messages also use.
const LONG_VALUE_CHARS: usize = 16;

/// The kinds serde reports for a received value (`serde::de::Unexpected`).
const RECEIVED_KINDS: &[&str] = &[
    "boolean",
    "integer",
    "floating point",
    "character",
    "string",
    "byte array",
    "unit value",
    "Option value",
    "newtype struct",
    "sequence",
    "map",
    "enum",
    "unit variant",
    "newtype variant",
    "tuple variant",
    "struct variant",
];

/// The redacted form of a codec error about `document`.
pub fn redact(message: &str, document: &DocumentText) -> String {
    let message = codec_values(&serde_values(message));
    // Look before numbers become `#`: that would split a value such as "Bob 2" and leave
    // "Bob" behind unrecognized.
    if message.chars().count() > MAX_MESSAGE_CHARS || document.appears_in(&message) {
        return WITHHELD.into();
    }
    numbers(&message)
}

/// Strings from the checked documents that could identify a player or carry text someone
/// typed: values of identifying fields, and every long string. A value without letters is
/// left out, because replacing numbers removes everything it could reveal.
pub struct DocumentText {
    values: BTreeSet<String>,
}

impl DocumentText {
    /// Collects from each document that reads as JSON; text that does not parse has no
    /// values to repeat.
    pub fn collect(documents: &[&str]) -> Self {
        let mut values = BTreeSet::new();
        for document in documents {
            let Ok(root) = serde_json::from_str::<Value>(document) else {
                continue;
            };
            let mut pending = vec![(&root, None)];
            while let Some((value, field)) = pending.pop() {
                match value {
                    Value::String(text) if identifying(field, text) => {
                        values.insert(text.clone());
                    }
                    Value::Array(items) => pending.extend(items.iter().map(|item| (item, field))),
                    Value::Object(fields) => pending
                        .extend(fields.iter().map(|(name, value)| (value, Some(name.as_str())))),
                    _ => {}
                }
            }
        }
        Self { values }
    }

    /// Whether a collected value occurs in `message` as a whole word or phrase, outside the
    /// placeholders.
    fn appears_in(&self, message: &str) -> bool {
        let mut probe = message.to_owned();
        for placeholder in PLACEHOLDERS {
            probe = probe.replace(placeholder, " ");
        }
        self.values.iter().any(|value| {
            probe.match_indices(value.as_str()).any(|(start, _)| {
                let before = probe[..start].chars().next_back();
                let after = probe[start + value.len()..].chars().next();
                !before.is_some_and(continues_word) && !after.is_some_and(continues_word)
            })
        })
    }
}

fn identifying(field: Option<&str>, text: &str) -> bool {
    let length = text.chars().count();
    let named = field
        .is_some_and(|field| IDENTIFYING_FIELDS.contains(&field) || field.ends_with("PlayerIds"));
    text.chars().any(char::is_alphabetic)
        && length <= MAX_MESSAGE_CHARS
        && (named || length >= LONG_VALUE_CHARS)
}

fn continues_word(letter: char) -> bool {
    letter.is_alphanumeric() || letter == '_'
}

/// Serde's received values. The text after the last `, expected` is the game's own
/// description of the type it wanted, so the received value ends there.
fn serde_values(message: &str) -> String {
    let message = replace_received(message, "invalid type: ", &[", expected "], received_kind);
    let message = replace_received(&message, "invalid value: ", &[", expected "], received_kind);
    let message = replace_received(
        &message,
        "unknown variant `",
        &["`, expected ", "`, there are no "],
        |_| "<value>",
    );
    replace_received(
        &message,
        "unknown field `",
        &["`, expected ", "`, there are no "],
        |_| "<field>",
    )
}

/// Replaces what follows `marker` up to the last of `ends`, or the rest of the message when
/// no end follows.
fn replace_received(
    message: &str,
    marker: &str,
    ends: &[&str],
    placeholder: fn(&str) -> &'static str,
) -> String {
    let Some(start) = message.find(marker) else {
        return message.to_owned();
    };
    let (kept, received) = message.split_at(start + marker.len());
    match ends.iter().filter_map(|end| received.rfind(end)).max() {
        Some(end) => format!("{kept}{}{}", placeholder(&received[..end]), &received[end..]),
        None => format!("{kept}{}", placeholder("")),
    }
}

/// The kind serde wrote before a received value, such as `string` in `string "Bob"`.
fn received_kind(received: &str) -> &'static str {
    let kind = received.split(['`', '"']).next().unwrap_or_default().trim();
    RECEIVED_KINDS.iter().find(|known| **known == kind).copied().unwrap_or("<value>")
}

/// The codec's messages that repeat a document key, tag, name or identity.
fn codec_values(message: &str) -> String {
    for (marker, placeholder) in
        [(" has unexpected field ", "<field>"), (" has unknown value ", "<value>")]
    {
        if let Some(start) = message.find(marker) {
            return format!("{}{marker}{placeholder}", &message[..start]);
        }
    }
    if let Some(path) = message.strip_suffix(" is not allowed") {
        return match path.rfind('.') {
            Some(dot) => format!("{}.<field> is not allowed", &path[..dot]),
            None => "<field> is not allowed".into(),
        };
    }
    for (start, end) in [
        ("legacy Goodie ", " is absent from its Boneyard scene"),
        ("game save secondary player ", " must be an object"),
    ] {
        if let Some(redacted) = replace_between(message, start, end, "<id>") {
            return redacted;
        }
    }
    if message.ends_with(": Boneyard bounds must have positive size") {
        return "<scene>: Boneyard bounds must have positive size".into();
    }
    if message.ends_with(" has an invalid link mask") && message.contains(": Road ") {
        return "<scene>: Road <id> has an invalid link mask".into();
    }
    if let Some(rest) = message.strip_prefix("Cannot load native navigation ") {
        let error = rest.rfind(".json: ").map_or("", |end| &rest[end + ".json: ".len()..]);
        return format!("Cannot load native navigation <file>: {error}");
    }
    message.to_owned()
}

/// Replaces the text between `start` and the last following `end`.
fn replace_between(message: &str, start: &str, end: &str, placeholder: &str) -> Option<String> {
    let first = message.find(start)? + start.len();
    let last = first + message[first..].rfind(end)?;
    Some(format!("{}{placeholder}{}", &message[..first], &message[last..]))
}

/// Replaces every number with `#`. Digits inside a name such as `handCast1` stay.
fn numbers(message: &str) -> String {
    #[derive(Clone, Copy)]
    enum Run {
        Gap,
        Name,
        Number,
    }
    let mut redacted = String::with_capacity(message.len());
    let mut run = Run::Gap;
    for letter in message.chars() {
        run = match run {
            _ if !(letter.is_ascii_alphanumeric() || letter == '_') => {
                redacted.push(letter);
                Run::Gap
            }
            Run::Number => Run::Number,
            Run::Gap if letter.is_ascii_digit() => {
                redacted.push('#');
                Run::Number
            }
            Run::Gap | Run::Name => {
                redacted.push(letter);
                Run::Name
            }
        };
    }
    redacted
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nothing() -> DocumentText {
        DocumentText { values: BTreeSet::new() }
    }

    #[test]
    fn static_codec_messages_pass_through_unchanged() {
        for message in [
            "game save is not valid JSON",
            "game save schema version is invalid",
            "game save player economy inventory is invalid or Hagatha state is invalid",
            "game save profile is missing field advancedUnlocks",
            "HubEconomyState: missing field `nextItemId`",
            "game save has unsafe field constructor",
        ] {
            assert_eq!(redact(message, &nothing()), message);
        }
    }

    #[test]
    fn serde_keeps_the_expected_type_but_not_the_received_value() {
        for (message, expected) in [
            (
                r#"HubEconomyState: invalid type: string "Bob", expected u64"#,
                "HubEconomyState: invalid type: string, expected u64",
            ),
            (
                "Rng: invalid value: integer `4294967297`, expected u32",
                "Rng: invalid value: integer, expected u32",
            ),
            (
                r#"invalid type: string "a, expected b", expected a sequence"#,
                "invalid type: string, expected a sequence",
            ),
            (
                "unknown variant `b`, `c`, expected one of `fire`, `water`",
                "unknown variant `<value>`, expected one of `fire`, `water`",
            ),
            (
                "unknown field `x`, there are no fields",
                "unknown field `<field>`, there are no fields",
            ),
            ("invalid type: string \"cut off", "invalid type: <value>"),
            (
                "Boneyard scene: invalid type: map, expected u32 at line 1 column 52",
                "Boneyard scene: invalid type: map, expected u32 at line # column #",
            ),
        ] {
            assert_eq!(redact(message, &nothing()), expected, "{message}");
        }
    }

    #[test]
    fn codec_templates_replace_document_keys_tags_names_and_identities() {
        for (message, expected) in [
            ("game save has unexpected field secret", "game save has unexpected field <field>"),
            (
                "game save Hagatha runtime 0 has unexpected field secret",
                "game save Hagatha runtime # has unexpected field <field>",
            ),
            (
                "HubEconomyState: HubInventoryItem: ItemRarity has unknown value Mythic Bob",
                "HubEconomyState: HubInventoryItem: ItemRarity has unknown value <value>",
            ),
            (
                "game save player economy.actionFeedback.skillBookOutcome.secret is not allowed",
                "game save player economy.actionFeedback.skillBookOutcome.<field> is not allowed",
            ),
            (
                "legacy Goodie grave of Bob is absent from its Boneyard scene",
                "legacy Goodie <id> is absent from its Boneyard scene",
            ),
            (
                "game save secondary player player-AbC must be an object",
                "game save secondary player <id> must be an object",
            ),
            (
                "Bob's Yard: Boneyard bounds must have positive size",
                "<scene>: Boneyard bounds must have positive size",
            ),
            (
                "Bob's Yard: Road road-9 has an invalid link mask",
                "<scene>: Road <id> has an invalid link mask",
            ),
            (
                "Cannot load native navigation /srv/game/assets/navigation/ab12.json: No such file or directory (os error 2)",
                "Cannot load native navigation <file>: No such file or directory (os error #)",
            ),
        ] {
            assert_eq!(redact(message, &nothing()), expected, "{message}");
        }
    }

    #[test]
    fn numbers_become_placeholders_but_names_keep_their_digits() {
        assert_eq!(
            numbers("game save Boneyard enemy actor 12 id is outside [1,9007199254740991]"),
            "game save Boneyard enemy actor # id is outside [#,#]"
        );
        assert_eq!(numbers("handCast1 sha256 -3.5 0x1f"), "handCast1 sha256 -#.# #");
    }

    #[test]
    fn a_message_that_still_repeats_document_text_is_withheld() {
        let document =
            DocumentText::collect(&[r#"{"summary":{"character":{"displayName":"Lady Bob"},
            "playerId":"p"},"notes":"a long free-text sentence","kind":"active",
            "items":[{"name":"value"},{"name":"Sack 2"},{"name":"12"}]}"#]);
        assert_eq!(redact("Lady Bob cannot be loaded", &document), WITHHELD);
        assert_eq!(redact("player p is absent", &document), WITHHELD);
        assert_eq!(redact("saw a long free-text sentence here", &document), WITHHELD);
        assert_eq!(redact("item Sack 2 is invalid", &document), WITHHELD);
        // A value without letters is covered by replacing numbers.
        assert_eq!(redact("item 12 is invalid", &document), "item # is invalid");
        // Short tags, words inside other words and placeholders are not document text.
        assert_eq!(
            redact("game save active run is invalid", &document),
            "game save active run is invalid"
        );
        assert_eq!(redact("Lady Bobbin is invalid", &document), "Lady Bobbin is invalid");
        assert_eq!(
            redact("unknown variant `zz`, expected one of `fire`", &document),
            "unknown variant `<value>`, expected one of `fire`"
        );
        assert_eq!(redact(&"x".repeat(MAX_MESSAGE_CHARS + 1), &nothing()), WITHHELD);
    }
}
