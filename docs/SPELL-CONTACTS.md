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

The canonical secondary presentation plan also consumes the retained private
animation list. It draws all six Skeleton/Archer/Mage head banks, all three
Zombie banks and Demon entries 80–97, plus DeadHawg debris 177–179. Each private
draw inherits field opacity and keeps its own unit scale. Captured images
descend as their timer falls; strictly below one, BadGuys 9 and 8 surround the
image clipped to the field-local `[-100, -110, 200, 110]` rectangle. This clip
does not move with the image offset. The web consumer shares the enemy view's
existing Demon atlas and the native mesh rectangle clipper. Unclipped secondary
sprites keep their original geometry. Drawing does not advance capture state,
change its admission, or create pulses.

Steam Jet's normal particles keep their contact lane when mana is empty or
insufficient. Handler `0x00542D20` uses half the resolved base damage for weak
casts and quarters alpha;
`0x00543266` tests the caster bank before granting contact byte `+0x50`,
independently of its low-mana local byte. Weak forces normal rather than Over.
Only Over remains cosmetic. Particle tick `0x0045B940` installs the ten-tick
target-owned `Mod_Steamed` while life is strictly above .5; its tick
`0x00625F40` supplies Fire damage. Learned Fire payloads survive the weak branch;
existing strong particles and modifiers keep their accepted payload after the
caster's reserve changes. Retained native ledger123 and its report133 instruction
catalog supersede the earlier weak-no-contact assertion. Independent constructor,
clock and real held-input mob regressions protect both participant identities.

Steam's per-tick mana debit and funded particle damage also use the current
casting-rate/class factor (`0x00656580(7)`). Weak damage uses a fixed .5 multiplier
on the already resolved damage cache; it does not multiply by casting rate.
`0x0054A633` calls the offensive resolver before caching that base, so equipment
and global damage boosts still apply. Independent rate/reserve/boost examples
cover the admission threshold, accepted cost and particle damage. Staff, Wand
and bare Constant use the shared Steam casting-rate authority; the separate bare
one-shot hand pose rate does not apply. Reserve equal to the scaled debit leaves
zero and selects weak, including both .5 and 2 casting rates in the native matrix.

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
