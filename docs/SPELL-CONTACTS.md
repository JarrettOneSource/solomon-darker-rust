# Spell contact rules

The original Solomon Dark 0.72.5 damage receiver owns player rescue regardless
of which spell supplied the damage. Its Cheat Death branch inserts the local
`CHEAT DEATH!` notification at `0x005304B0`, inside Player damage `0x0052F540`.
The recovered four-sound program and rescue state are recorded in the retained
native Hagatha ledger, entry 175. An Ether Drain contact reaches that receiver
through `0x00548150`; its flags `0x10A` suppress ordinary hit feedback, rather
than the accepted rescue notification.

In Rust, `session::player_contacts` applies the receiver and returns feedback
events. Ether Drain retains its accepted `PlayerCheatDeath` event in the Boneyard
stream while preserving its ordinary hit-feedback suppression. The shared
notification and audio consumers use this event; the receiver separately owns
restored health, protection, flash, and rescue wave.

Ether Drain's living-player contact is distinct from capture. Retail contact
`0x005F8620` damages and moves the player without invoking the capture callback.
Gold/Sack retirement strictly inside radius 10 calls `0x005EE840(1)`; player
maintenance `0x00533AAA` calls it with 2 only for an eligible dying corpse. Private
capture animations and eligible grounded world animations have their own pulse
programs. A living player at the center does not acquire those capture semantics.
These memberships and constants are recorded in retained native ledger entry 083.

Steam Jet has a separate low-mana contract. Handler `0x00542D20` creates weak
normal particles at quarter alpha. Only a full-power normal particle owns the
contact clock that installs target-owned `Mod_Steamed` through particle tick
`0x0045B940`; weak and Over particles have no such contact lane. The modifier's
tick `0x00625F40` supplies Fire damage. This recovered contract, including the
weak/Over exclusion, is recorded in retained native ledger entry 123. Reduced
damage from other primary spells does not imply a Steam damage rule.

The retained original-executable ledgers are under the Website repository's
`docs/Game Native Parity RE/`. The Rust build uses its tracked inputs. Frozen
fixtures retain their recorded provenance as described in
[Reference data](REFERENCE-DATA.md).
