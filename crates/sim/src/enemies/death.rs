//! Terminal brain entry from Website `enemies/deaths.ts::deathBrain`.

use super::model::*;

pub(super) fn death_brain(brain: &mut EnemyBrain) {
    match brain {
        EnemyBrain::Skeleton(brain) => {
            brain.pike = None;
            brain.phase = SkeletonPhase::Death;
        }
        EnemyBrain::Archer(brain) => brain.phase = ArcherPhase::Death,
        EnemyBrain::Mage(brain) => {
            brain.lightning_target_player_id = None;
            brain.lightning_target_position = None;
            brain.lightning_ticks_remaining = 0;
            brain.phase = MagePhase::Death;
        }
        EnemyBrain::Imp(brain) => brain.phase = FlightPhase::Death,
        EnemyBrain::Zombie(brain) => brain.phase = ZombiePhase::Death,
        EnemyBrain::Wraith(brain) => brain.phase = FlightPhase::Death,
        EnemyBrain::Demon(brain) => brain.phase = DemonPhase::Death,
        EnemyBrain::Coffin(brain) => brain.phase = CoffinPhase::Death,
        EnemyBrain::Portal(brain) => brain.phase = PortalPhase::Death,
        EnemyBrain::DemonSkull(brain) => {
            brain.actions.clear();
            brain.phase = BossPhase::Death;
        }
        EnemyBrain::Faculty(brain) => {
            brain.action = None;
            brain.body_pose = 0.0;
            brain.hand_mask = 0;
            brain.heading_locked = false;
            brain.lightning_active = false;
            brain.phase = BossPhase::Death;
        }
        EnemyBrain::Heartmonger(brain) => brain.phase = HeartmongerPhase::Death,
        EnemyBrain::Spider(brain) => brain.phase = SpiderPhase::Death,
        EnemyBrain::Cocoon(brain) => brain.phase = CocoonPhase::Death,
    }
}
