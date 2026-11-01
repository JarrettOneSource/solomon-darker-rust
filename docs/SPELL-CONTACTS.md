# Spell contact rules

The original Solomon Dark 0.72.5 damage receiver owns player rescue regardless
of which spell supplied the damage. Its Cheat Death branch inserts the local
`CHEAT DEATH!` notification at `0x005304B0`, inside Player damage `0x0052F540`.
The recovered four-sound program and rescue state are recorded in the retained
native Hagatha ledger, entry 175. An Ether Drain contact reaches that receiver
through `0x00548150`; its flags `0x10A` suppress ordinary hit feedback, rather
than the accepted rescue notification.

The same receiver's until-hurt charm branch at `0x0052FCEA–0x0052FDEB` plays
the loss stream and clears Reverie/Serendipity before common Actor reaction at
`0x0052FDF1`. That reaction tests `flags & 0x08` at `0x00627FA4` to suppress the
redraw latch; the charm-loss branch has no such gate. The retained native ledger,
entry 299, records this distinction and the deliberate brief loss-text addition.

In Rust, `session::player_contacts` applies the receiver and returns feedback
events. Ether Drain retains exactly its accepted `PlayerCharmLost` and
`PlayerCheatDeath` events in the Boneyard stream while preserving ordinary
Ouch/status feedback suppression. The shared notification and audio consumers
use these events; the receiver separately owns cleared charm flags, restored
health, protection, flash, and rescue wave. `spell_contact_rules` checks real
caster/peer field contacts, each until-hurt charm, combined loss/rescue ordering,
and suppression of ordinary hurt sound.

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

Ethereal Boulder uses the shared Earth movement/contact step before its own
retail guidance tail (`0x00621450`). Release `0x005FA6D0` acquires independently
at each rock's root through `0x005F28A0`; More Missiles caps the authored count at
four. Smart Missiles scales turn input and the reacquisition window, while the
original flight speed is `3 * .75 = 2.25`. Extra children use their constructor
turn input with the native `.95`/`.9` template factors. A lost non-null handle
may reacquire; a null handle never polls, and an already-hit closest target is
rejected without a farther fallback. Resolved inactive targets supply one final
steering sample after accepted contacts. The countdown limits steering; it does
not retire an otherwise live rock. Terrain, exhausted contact pools, bounds and
scene/owner teardown retain their own retirement paths.

`weld::persistent` owns this state and the contact transaction commits guidance
after real enemy acceptance. Website schema52 and native checkpoints preserve
heading, speed, target, policy, window and accumulated turn state. Older disk
records retain identity, pose, stored velocity and pools, defaulting the absent
handle/accumulator to the native null target/`.01`; their unrecorded target history
cannot be recovered. Fresh release uses the corrected owner. Gameplay protocol3
requires the new state; deployment protocol2 is unchanged. Native ledger123 and
`ethereal_boulder_guidance` record the authored scalar and sibling contract;
`spell_contact_rules` exercises both funded and weak production release callers.

The retained original-executable ledgers are under the Website repository's
`docs/Game Native Parity RE/`. The Rust build uses its tracked inputs. Frozen
fixtures retain their recorded provenance as described in
[Reference data](REFERENCE-DATA.md).
