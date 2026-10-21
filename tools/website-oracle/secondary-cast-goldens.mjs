import { execFileSync } from 'node:child_process'
import { loadKernel, websiteDir, writeFixture } from './oracle.mjs'
import { allLearnedBook, factors, authority, context } from './secondary-inputs.mjs'
const sourceRevision=execFileSync('git',['rev-parse','HEAD'],{cwd:websiteDir,encoding:'utf8'}).trim()
const secondary = await loadKernel('core-kernels/native-secondary-abilities.ts')
const profile = await loadKernel('core-kernels/player-progression.ts')
const contract = await loadKernel('core-kernels/native-secondary-ability-contract.ts')
const belt = await loadKernel('core-kernels/native-belt.ts')
const targets = [
  { family: 'SKELETON', id: 7, position: { x: 30, y: 0 }, radius: 15, scale: 1, shieldHealth: 0 },
  { family: 'DEMON', id: 8, position: { x: 90, y: 0 }, radius: 25, scale: 1.2, shieldHealth: 0 },
  { family: 'ZOMBIE', id: 9, position: { x: 250, y: 0 }, radius: 20, scale: 1, shieldHealth: 0 },
].map(target => ({ ...target, nativeFlags: 2, lightRegistration: { managerLane: 'actor', registrationOrdinal: 100 + target.id } }))
const dampen = { casterTargetIds: [7], shieldTargetIds: [8, 9], projectiles: [
  { id: 8, kind: 'firebolt', payload: 'fire', position: { x: 10, y: 0 } },
  { id: 9, kind: 'guided-missile', payload: 'cold', position: { x: 0, y: 20 } },
  { id: 10, kind: 'guided-missile', payload: 'poison', position: { x: 5, y: 20 } },
  { id: 11, kind: 'skull-missile', payload: 'dark', position: { x: 10, y: 20 } },
  { id: 12, kind: 'dark-fireball', payload: 'dark', position: { x: 15, y: 20 } },
] }
const scenery = [{ id: 1, typeId: 2001, position: { x: 50, y: 0 } }, { id: 2, typeId: 2009, position: { x: 20, y: 100 } }]
const cases = []
function record(skillId, mode, book = allLearnedBook()) {
  const player = authority(book, factors(mode === 'equipped'))
  player.belt = belt.bindNativeBeltSkill(player.belt, book, skillId, 0)
  player.input = { ...player.input, aim: { x: 100, y: 0 }, cast: { primary: false, quickbar: 0 } }
  if (mode === 'maximum') {
    player.maximumGolem = true; player.maximumLeviathan = true; player.maximumMagicStorm = true
    player.maximumRingOfFire = true; player.maximumRingOfIce = true
  }
  if (mode === 'enhanced') player.enhancedEffects = true
  if (mode === 'low-mana') player.currentMana = 0
  if (mode === 'ineligible') player.eligible = false
  if (mode === 'focus') player.focusInstantRechargeChancePercent = 100
  const initialPlayer = secondary.createNativeSecondaryPlayerState()
  if (mode === 'busy') initialPlayer.castAction = { weaponKind: 'staff', progress: 1 }
  if (mode === 'global-cooldown') initialPlayer.globalCooldownTicks = 2
  if (mode === 'skill-cooldown') {
    const ticks = [...initialPlayer.cooldownTicksBySkill]
    ticks[skillId] = 300
    initialPlayer.cooldownTicksBySkill = ticks
  }
  const state = { ...secondary.createNativeSecondarySimulation(77), players: { player: initialPlayer } }
  const world = { ...context(player),
    dampenCandidates: () => dampen,
    phasingDestination: () => mode === 'blocked-phase' ? null : { x: 20, y: 0 },
    target: (_world, id) => targets.find(target => target.id === id) ?? null,
    targets: (_world, center, radius) => targets.filter(target => Math.hypot(target.position.x - center.x, target.position.y - center.y) <= radius + target.radius),
    sceneryTargets: () => scenery,
  }
  const result = secondary.stepNativeSecondaryAbilities(state, world)
  const { state: ignored, ...outbox } = result
  cases.push({ name: `${skillId}-${mode}-${book.weldBuildId ?? 'plain'}`, skillId, mode,
    authority: player, initial: state, targets, dampen, scenery,
    result: outbox, state: result.state })
}
for (const id of contract.NATIVE_SECONDARY_ABILITY_IDS) {
  for (const mode of ['normal', 'equipped', 'maximum', 'enhanced', 'low-mana', 'ineligible', 'focus', 'busy', 'global-cooldown', 'skill-cooldown']) record(id, mode)
}
record(15, 'blocked-phase')
for (const build of profile.NATIVE_WELD_BUILDS) record(50, 'normal', profile.grantPlayerWeldBuild(allLearnedBook(), build.id))
writeFixture('secondary-cast-goldens.json', 'tools/website-oracle/secondary-cast-goldens.mjs',
  'Current Website/frontend/src/game/core-kernels/native-secondary-abilities.ts', { sourceRevision, cases })
