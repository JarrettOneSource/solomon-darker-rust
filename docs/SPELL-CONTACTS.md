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
