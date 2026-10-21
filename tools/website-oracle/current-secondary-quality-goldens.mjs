// Source global quality can differ from the retained actor and owner birth flags.
import { execFileSync } from 'node:child_process'
import { loadKernel, websiteDir, writeFixture } from './oracle.mjs'
import { allLearnedBook, authority, context, factors } from './secondary-inputs.mjs'
const secondary = await loadKernel('core-kernels/native-secondary-abilities.ts')
const belt = await loadKernel('core-kernels/native-belt.ts')
const manager = await loadKernel('core-kernels/native-world-manager-order.ts')
const sourceRevision = execFileSync('git', ['rev-parse', 'HEAD'], { cwd: websiteDir, encoding: 'utf8' }).trim()
const actorKeys = ['ageTicks','alpha','damage','enhanced','endpoint','frame','freezeTicks','golem',
  'hitTargetIds','id','kind','lifetimeTicks','lightRegistration','midpoint','miscLightAppendOrdinal',
  'ownerId','painterRegistrations','phase','position','presentationRng','quantity','radius','rank',
  'rotationRadians','scale','skillId','slowFactor','targetId','variant','velocity','worldKey']
const cases = []
for (const skillId of [11, 12, 27, 35, 41, 45, 46, 72, 74, 76]) for (const initialQuality of [false, true]) {
  let player = authority(allLearnedBook(), factors(false))
  player = { ...player, enhancedEffects: !initialQuality,
    belt: belt.bindNativeBeltSkill(player.belt, player.skillBook, skillId, 0) }
  const originalPlayer = structuredClone(player)
  let state = secondary.createNativeSecondarySimulation(91)
  const order = manager.createNativeWorldManagerOrder()
  const frames = []
  for (let tick = 1; tick <= 420; tick += 1) {
    player = { ...player, input: { ...player.input, aim: { x: 100, y: 0 },
      cast: { primary: skillId === 12 && tick % 40 < 20, quickbar: tick === 1 ? 0 : null } } }
    const result = secondary.stepNativeSecondaryAbilities(state, { ...context(player,tick),
      enhancedEffects: tick <= 200 ? initialQuality : !initialQuality,
      registerWorldPainter: order.register,
    })
    state = result.state
    if ([1,2,3,50,100,199,200,201,202,250,350,420].includes(tick)) {
      frames.push({ tick, expected: { actors: state.actors.map(actor => Object.fromEntries(actorKeys.map(key=>[key,actor[key]]))),
        player: state.players.player, events: state.events, rng: state.rng,
        nextActorId:state.nextActorId, nextEventId:state.nextEventId,
        order:order.state().nextRegistrationOrdinal } })
    }
  }
  cases.push({ skillId, initialQuality, player:originalPlayer, frames })
}
writeFixture('current-secondary-quality-goldens.json','tools/website-oracle/current-secondary-quality-goldens.mjs',
  'current native-secondary-abilities.ts global quality during actual casts, retained actors and subsequent births',
  { sourceRevision, cases })
