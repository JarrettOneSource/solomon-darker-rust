// The ordinary College wizard has no selected primary: attachment 4 and fixed robe 13.
import { loadKernel, point, writeFixture } from './oracle.mjs'

const presentation = await loadKernel('player-character-presentation.ts')
const samples = []
for (let heading = 0; heading < 24; heading += 1) {
  for (const gait of [0, 90, 180, 270]) {
    for (const walkCycle of [0, 1.5, 4.999]) {
      const plan = presentation.createPlayerCharacterDrawPlan({
        config: { element: 'ether' }, headingIndex: heading, gaitDegrees: gait,
        velocity: { x: 0, y: 0 }, walkCyclePrimary: walkCycle,
        primaryCast: { selectedPrimaryId: -1, actionTick: -1, channelActive: false,
          oneShotAttackPoseHeld: false, weaponPulse: 0 },
      })
      samples.push({ heading, gait, walkCycle,
        fixedRobePose: presentation.playerCharacterRobeFixedPose(plan.attachmentPose, true, true),
        attachmentPose: presentation.NATIVE_UNSELECTED_PRIMARY_ATTACHMENT_POSE,
        robePose: plan.robePose, fixed: point(plan.fixedRobeOffset),
        front: point(plan.frontAttachmentOffset), head: point(plan.headOffset),
        unselected: plan.unselectedPrimaryAttachment,
      })
    }
  }
}
writeFixture('wizard-attachments-goldens.json', 'tools/website-oracle/wizard-attachments-goldens.mjs',
  'Website/frontend/src/game/player-character-presentation.ts', { samples })
