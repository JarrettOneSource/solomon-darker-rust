// Full native cast/flight journeys. Family fixtures already compare every actor
// field; this boundary records queue order, physical state, mana and shared RNG.
import { readFileSync } from 'node:fs'
import { execFileSync } from 'node:child_process'
import { resolve } from 'node:path'
import { fixturesDir, loadKernel, websiteDir, writeFixture } from './oracle.mjs'

const primary = await loadKernel('core-kernels/primary-spells.ts')
const character = await loadKernel('core-kernels/player-character.ts')
const random = await loadKernel('core-kernels/native-rng.ts')
const managers = await loadKernel('core-kernels/native-world-manager-order.ts')
const profiles = JSON.parse(readFileSync(resolve(fixturesDir, 'primary-profile-goldens.json'), 'utf8')).profiles
import { weldActorRow } from './weld-state-rows.mjs'
const point = value => value == null ? null : [value.x, value.y]
const target = { active: true, actorFlags: 2, attachment: { x: 0, y: -3 }, bodyRadius: 15,
  cellBindingOrder: 0, headingDeg: 0, id: 'enemy:1', kind: 'enemy', nativePriority: 0,
  pendingRemove: false, position: { x: 450, y: 200 }, queryLane: 'grid', registrationOrder: 0 }
const cases = []
const screens = []
for (let buildId = 1000; buildId <= 1009; buildId += 1) {
  const profile = profiles.find(value => value.name === `weld-${buildId}-frozen`).profile
  const scenarios = [{ name: 'held', mana: 2000, blocked: false }, { name: 'empty', mana: 0.05, blocked: false }]
  if ([1000, 1001, 1002, 1006, 1008, 1009].includes(buildId)) scenarios.push({ name: 'terrain', mana: 2000, blocked: true })
  for (const scenario of scenarios) {
    const release = 90
    const progress = [1006, 1008].includes(buildId) ? 2.6 : 1
    let player = character.createPlayerCharacter({ element: 'ether', discipline: 'arcane', displayName: 'Weld oracle' }, { x: 100, y: 200 })
    let spells = primary.createPrimarySpellSimulation()
    let rng = random.createNativeRng(731)
    let mana = scenario.mana
    const order = managers.createNativeWorldManagerOrder({ nextRegistrationOrdinal: { actor: 49, transient: 3 } })
    const frames = []
    for (let tick = 0; tick <= 320; tick += 1) {
      const result = primary.stepPrimarySpells({
        canPlaceProjectile: () => true, canTraverseProjectile: () => !scenario.blocked,
        castAuthority: { hero: { weaponKind: 'staff', alive: true, availableMana: mana,
          castProgressFactor: progress, eligible: tick < 260, planeActive: false, primarySkill: profile } },
        inputs: { hero: { aim: { x: 700, y: 200 }, cast: { primary: tick < release, quickbar: null },
          movement: { x: 0, y: 0 }, viewportWidth: 800, viewportHeight: 600 } },
        players: { hero: player }, previousPlayers: { hero: player }, registerWorldPainter: order.register,
        rng, spells, tick, viewScale: 1.2,
        spellObstructionPoint: (_owner, start, end) => scenario.blocked ? { x: (start.x + end.x) / 2, y: (start.y + end.y) / 2 } : null,
        spellRangeEndpoint: (_owner, start, direction) => ({ x: start.x + direction.x * 600, y: start.y + direction.y * 600 }),
        spellTargets: () => [target], worldKeyForPlayer: () => 'boneyard:weld',
      })
      player = result.players.hero; spells = result.spells; rng = result.rng
      mana -= result.manaSpent.hero
      if ([0, 1, 18, 19, 20, 29, 30, 59, 79, 89, 90, 91, 120, 160, 240, 320].includes(tick)) {
        const cast = player.primaryCast
        frames.push({ tick, mana, spent: result.manaSpent.hero, underflow: result.manaUnderflowPlayerIds.length > 0,
          heading: player.headingIndex, cast: [cast.actionTick, point(cast.aimDirection), cast.castSequence,
            cast.channelActive, cast.emissionSequence, cast.etherBlastCharge, cast.etherBlastChargeCueSequence,
            cast.fizzleSequence, cast.held, cast.lastWeldPlaybackRate, cast.lastWeldSoundVariant,
            cast.oneShotAttackPoseHeld, cast.selectedPrimaryAgeTicks, cast.selectedPrimaryId, cast.targetId, cast.underpowered, cast.weaponPulse],
          spells: { nextId: spells.nextId, projectiles: spells.projectiles.map(weldActorRow), transients: spells.transients.map(weldActorRow) },
          rng, order: order.state().nextRegistrationOrdinal,
          emissions: result.channelEmissions.map(emission => [emission.damage, point(emission.direction), point(emission.endpoint),
            emission.id, emission.kind, emission.manaCost, point(emission.origin), point(emission.queryOrigin), emission.terrainContact, emission.underpowered]),
        })
      }
      const screenTick = [1003, 1004, 1005].includes(buildId) ? 59 : [1006, 1008].includes(buildId) ? 79 : buildId === 1007 ? 89 : 29
      if (scenario.name === 'held' && (tick === screenTick || ([1006, 1008].includes(buildId) && tick === 110) || (buildId === 1007 && tick === 150))) screens.push({ buildId, tick, spells })
    }
    cases.push({ ...scenario, name: `weld-${buildId}-${scenario.name}`, profile, release, progress, frames })
  }
}
writeFixture('weld-step-goldens.json', 'tools/website-oracle/weld-step-goldens.mjs',
  'Website/frontend/src/game/core-kernels/primary-spells.ts', {
    sourceRevision: execFileSync('git', ['-C', websiteDir, 'rev-parse', 'HEAD'], {encoding: 'utf8'}).trim(), cases, screens,
  })
