import { execFileSync } from 'node:child_process'
import { loadKernel, websiteDir, writeFixture } from './oracle.mjs'
const spells = await loadKernel('core-kernels/primary-spells.ts')
const character = await loadKernel('core-kernels/player-character.ts')
const progression = await loadKernel('core-kernels/player-progression.ts')
const primary = await loadKernel('core-kernels/native-primary-skill-profile.ts')
const random = await loadKernel('core-kernels/native-rng.ts')
const { boneyardWorldLightQuery } = await loadKernel('core-server/boneyard-world-light.ts')
import { point, projectileRow, transientRow } from './primary-step-rows.mjs'
const scenarios = []
for (const element of ['air', 'earth', 'ether', 'fire', 'water']) {
  scenarios.push({ element, name: `${element}-held`, mana: 100, release: element === 'earth' ? 150 : 80 })
  scenarios.push({ element, name: `${element}-empty`, mana: 0.05, release: 30 })
}
scenarios.push({ element: 'earth', name: 'earth-early-release', mana: 100, release: 10 })
scenarios.push({ element: 'ether', name: 'ether-charged', mana: 100, release: 80, charge: 2.8 })
scenarios.push({ element: 'fire', name: 'fire-terrain', mana: 100, release: 80, blocked: true })
scenarios.push({ element: 'fire', name: 'fire-exact-mana', mana: 12, release: 30 })
const target = { id: 'enemy:1', position: { x: 450, y: -150 }, active: true, actorFlags: 2,
  attachment: { x: 0, y: -3 }, bodyRadius: 15, cellBindingOrder: 0, headingDeg: 0,
  kind: 'enemy', nativePriority: 0, pendingRemove: false, queryLane: 'grid', registrationOrder: 0 }
const cases = []
for (const scenario of scenarios) {
  const config = { element: scenario.element, discipline: 'arcane', displayName: 'Primary step oracle' }
  let player = character.createPlayerCharacter(config, { x: 100, y: 200 })
  if (scenario.charge) player.primaryCast.etherBlastCharge = scenario.charge
  const book = progression.createPlayerSkillBook(config)
  const profile = primary.nativePrimarySkillProfile(book, progression.playerStatBook(), { damage: 1, manaCost: 1 })
  let state = spells.createPrimarySpellSimulation()
  let rng = random.createNativeRng(7)
  let mana = scenario.mana
  let actor = 49
  let transient = 3
  const frames = []
  for (let tick = 0; tick <= 180; tick += 1) {
    const result = spells.stepPrimarySpells({
      canPlaceProjectile: () => true, canTraverseProjectile: () => !scenario.blocked,
      castAuthority: { player: { weaponKind: 'staff', alive: true, availableMana: mana,
        castProgressFactor: 1, eligible: tick < 170, planeActive: false, primarySkill: profile } },
      inputs: { player: { aim: { x: 400, y: -100 }, cast: { primary: tick < scenario.release, quickbar: null },
        movement: { x: 0, y: 0 }, viewportWidth: 800, viewportHeight: 600 } },
      players: { player }, previousPlayers: { player },
      registerWorldPainter: managerLane => ({ managerLane, registrationOrdinal: managerLane === 'actor' ? actor++ : transient++ }),
      rng, spells: state, tick, viewScale: 1.2,
      spellObstructionPoint: (_owner, start, end) => scenario.blocked
        ? { x: (start.x + end.x) * 0.5, y: (start.y + end.y) * 0.5 } : null,
      spellRangeEndpoint: (_owner, start, direction) => ({ x: start.x + direction.x * 600, y: start.y + direction.y * 600 }),
      spellTargets: () => [target], worldKeyForPlayer: () => 'boneyard:oracle',
    })
    player = result.players.player; state = result.spells; rng = result.rng
    mana -= result.manaSpent.player
    if ([0, 1, 13, 14, 18, 30, 59, 60, 95, 120, 149, 150, 151, 170, 180].includes(tick)) {
      const cast = player.primaryCast
      // Exercise the collector with real retained spell actors, including zero-age Air
      // MiscLights. The separate manager fixture covers multiple participant cameras.
      const lightQuery = boneyardWorldLightQuery({
        bounds: { x: 0, y: 0, w: 2400, h: 3700 }, runId: 'oracle',
        arenaTransition: null, lanternPosition: null, lanternLightRegistration: null,
      }, { player }, {
        actors: [], deathEffects: [], bossSpells: [], projectiles: [],
        projectileEffects: [], mageLightningPulses: [],
      }, tick, {
        inputs: { player: { viewportWidth: 800, viewportHeight: 600 } },
        playerEntities: { identities: [{ playerId: 'player' }], lightings: [{
          overlayEffectPhase: 0.25, lightRegistration: { managerLane: 'actor', registrationOrdinal: 48 },
        }] }, primarySpells: state,
      })
      const lightSamples = [player.position, { x: 250, y: 50 }, { x: 450, y: -150 },
        { x: 600, y: 300 }, { x: 2000, y: 2500 }].map(position => lightQuery.scalarAt(position))
      frames.push({ tick, mana, spent: result.manaSpent.player, underflow: result.manaUnderflowPlayerIds.length > 0,
        heading: player.headingIndex, cast: [cast.actionTick, point(cast.aimDirection), cast.castSequence,
          cast.channelActive, cast.emissionSequence, cast.etherBlastCharge, cast.etherBlastChargeCueSequence,
          cast.fizzleSequence, cast.held, cast.oneShotAttackPoseHeld, cast.selectedPrimaryAgeTicks,
          cast.selectedPrimaryId, cast.targetId, cast.underpowered, cast.weaponPulse],
        state: { nextId: state.nextId, projectiles: state.projectiles.map(projectileRow), transients: state.transients.map(transientRow) },
        rng, order: [actor, transient], emissions: result.channelEmissions.map(e => [e.damage, point(e.direction), point(e.endpoint), e.id, e.kind, e.manaCost, point(e.origin), point(e.queryOrigin), e.underpowered]),
        light: { cameras: lightQuery.cameras, acceptedSources: lightQuery.acceptedSources, samples: lightSamples },
      })
    }
  }
  cases.push({ ...scenario, frames })
}
writeFixture('primary-step-goldens.json', 'tools/website-oracle/primary-step-goldens.mjs', 'Website/frontend/src/game/core-kernels/primary-spells.ts', {
  sourceRevision: execFileSync('git', ['-C', websiteDir, 'rev-parse', 'HEAD'], {encoding: 'utf8'}).trim(), cases,
})
