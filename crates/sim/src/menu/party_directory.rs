//! Party ID editing and directory actions from Website `party-join.ts` and
//! `party-directory.ts`. The broker still owns actual listings and admission.

use crate::social::PartyVisibility;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DirectoryPartyAction {
    Join,
    Request,
    Wait,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DirectoryPartyPresentation<'a> {
    pub location: &'a str,
    pub squad: String,
    pub status: &'static str,
}

#[must_use]
pub fn directory_party_presentation(
    member_count: usize,
    max_members: usize,
    boneyard_name: Option<&str>,
) -> DirectoryPartyPresentation<'_> {
    DirectoryPartyPresentation {
        location: boneyard_name.unwrap_or("COLLEGE COURTYARD"),
        squad: format!("{member_count} / {max_members}"),
        status: if boneyard_name.is_some() { "IN GAME" } else { "IN HUB" },
    }
}

#[must_use]
pub fn directory_party_action(visibility: PartyVisibility, playing: bool) -> DirectoryPartyAction {
    if playing {
        return DirectoryPartyAction::Wait;
    }
    if visibility == PartyVisibility::Public {
        DirectoryPartyAction::Join
    } else {
        DirectoryPartyAction::Request
    }
}

/// The source keeps the final eight admitted characters when pasting a longer ID.
#[must_use]
pub fn normalize_party_code(value: &str) -> String {
    let characters: Vec<char> = value
        .to_uppercase()
        .chars()
        .filter(|character| "ABCDEFGHJKLMNPQRSTUVWXYZ23456789".contains(*character))
        .collect();
    let start = characters.len().saturating_sub(8);
    let compact: String = characters[start..].iter().collect();
    if compact.len() <= 4 {
        return compact;
    }
    format!("{}-{}", &compact[..4], &compact[4..])
}

#[must_use]
pub fn complete_party_code(value: &str) -> bool {
    normalize_party_code(value).len() == 9
}
