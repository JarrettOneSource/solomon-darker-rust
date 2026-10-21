// Ordered multi-tick ability journeys against the unchanged shipping kernel.
import { execFileSync } from 'node:child_process'
import { loadKernel, websiteDir, writeFixture } from './oracle.mjs'
import { allLearnedBook, factors, authority, context } from './secondary-inputs.mjs'

const secondary = await loadKernel('core-kernels/native-secondary-abilities.ts')
const contract = await loadKernel('core-kernels/native-secondary-ability-contract.ts')
const belt = await loadKernel('core-kernels/native-belt.ts')
const harmful = await loadKernel('core-kernels/player-harmful-contact.ts')
const random = await loadKernel('core-kernels/native-rng.ts')
const sourceRevision = execFileSync('git', ['rev-parse', 'HEAD'], { cwd: websiteDir, encoding: 'utf8' }).trim()
const targets = [
  { family: 'SKELETON', id: 7, position: { x: 30, y: 0 }, radius: 15, scale: 1, shieldHealth: 0 },
  { family: 'DEMON', id: 8, position: { x: 90, y: 0 }, radius: 25, scale: 1.2, shieldHealth: 0 },
  { family: 'ZOMBIE', id: 9, position: { x: 250, y: 0 }, radius: 20, scale: 1, shieldHealth: 0 },
].map(target => ({ ...target, nativeFlags: 2, lightRegistration: { managerLane: 'actor', registrationOrdinal: 100 + target.id } }))
const dampen = { casterTargetIds: [7], shieldTargetIds: [8], projectiles: [
  { id: 8, kind: 'firebolt', payload: 'fire', position: { x: 10, y: 0 } },
  { id: 9, kind: 'guided-missile', payload: 'cold', position: { x: 0, y: 20 } },
  { id: 10, kind: 'guided-missile', payload: 'poison', position: { x: 5, y: 20 } },
  { id: 11, kind: 'skull-missile', payload: 'dark', position: { x: 10, y: 20 } },
] }
const scenery = [{ id: 1, typeId: 2001, position: { x: 50, y: 0 } }, { id: 2, typeId: 2009, position: { x: 20, y: 100 } }]
const sampleTicks = new Set([1, 2, 3, 5, 9, 10, 11, 20, 25, 30, 50, 75, 100, 150, 200, 250, 399, 400, 401, 500, 800, 1000, 1200, 1500, 1600, 1640, 1800, 2000, 2200])
const seenKinds = new Set()
const cases = []
function record(skillId, mode) {
  let player = authority(allLearnedBook(mode === 'maximum'), factors(mode === 'equipped'))
  player.belt = belt.bindNativeBeltSkill(player.belt, player.skillBook, skillId, 0)
  player.input = { ...player.input, aim: { x: 100, y: 0 }, cast: { primary: false, quickbar: 0 } }
  if (mode === 'enhanced') player.enhancedEffects = true
  if (mode === 'electric-trap') {
    const ranks = [...player.skillBook.effectiveRanks]
    ranks[24] = 1
    player.skillBook = { ...player.skillBook, effectiveRanks: ranks, primarySkillId: 24 }
  }
  if (mode === 'shield-explode') player.explosiveShieldDamage = 25
  if (mode === 'maximum') {
    player.maximumGolem = true; player.maximumLeviathan = true; player.maximumMagicStorm = true
    player.maximumRingOfFire = true; player.maximumRingOfIce = true
  }
  const initialAuthority = structuredClone(player)
  const liveTargets = structuredClone(targets)
  let state = secondary.createNativeSecondarySimulation(77)
  const initial = state
  const samples = []
  for (let tick = 1; tick <= 2200; tick++) {
    // Release each belt button after its initial press. Planewalker also sees
    // actual primary-button edges, rather than spawning a replacement fake shot.
    const pressed = tick === 1 || (mode === 'toggle-off' && (tick === 201 || tick === 751))
    player = { ...player, input: { ...player.input, cast: { quickbar: pressed ? 0 : null, primary: skillId === 12 && (tick % 200 < 100) } } }
    if (mode === 'live-equipment' && (tick === 800 || tick === 1500)) {
      player = { ...player, offensiveFactors: factors(tick === 800) }
    }
    if (mode === 'overload' && tick >= 200) player = { ...player, maximumMana: 1 }
    if (mode === 'ineligible-toggle' && tick >= 200) player = { ...player, eligible: false }
    if (mode === 'world-switch' && tick >= 30) player = { ...player, worldKey: 'boneyard:other' }
    if ((mode === 'shield-break' || mode === 'shield-explode') && tick === 2) {
      state = secondary.applyNativeSecondaryPlayerDamage(state, 'player', 100000, tick, { x: 0, y: 0 }, player.worldKey).state
    }
    if (mode === 'golem-death' && tick === 450) {
      state = secondary.applyNativeSecondaryGolemDamage(state, state.actors.find(actor => actor.kind === 'golem').id,
        { primaryDamage: 1000000, secondaryDamage: 0, reflectablePhysicalSourceInRange: true }, tick).state
    }
    if (mode === 'effects' && tick === 2) {
      state = secondary.applyNativeSecondaryEtherBurn(state, { ownerId: 'player', rank: 2, target: liveTargets[0], worldKey: player.worldKey })
      state = secondary.applyNativeSecondaryTargetEffect(state, player.worldKey, 7, {
        prismaticTicks: 1, frostBurnTicks: 1, frostBurnDamagePerTick: 0.01,
        frostBurnOwnerId: 'player', frostBurnSkillId: 35, frostBurnSourceActorId: 1,
        electricBurn: { arcCount: 2, damagePerTick: 1, ownerId: 'player', sourceActorId: 1, stunFactor: 0.8, ticks: 1 },
        steamed: { damagePerTick: 1, emberDamage: 2, emberFragments: 3, explodeDamage: 4, explodeRadius: 5, ownerId: 'player', sourceActorId: 1, ticks: 1 },
      })
    }
    const world = { ...context(player, tick), dampenCandidates: () => dampen,
      phasingDestination: () => ({ x: 20, y: 0 }),
      targetExists: (key, id) => key === 'boneyard:oracle' && liveTargets.some(target => target.id === id),
      target: (_world, id) => liveTargets.find(target => target.id === id) ?? null,
      targets: (_world, center, radius) => liveTargets.filter(target => Math.hypot(target.position.x - center.x, target.position.y - center.y) <= radius + target.radius),
      // This public-kernel fixture declares independent unobstructed target roots.
      // The separate actual Boneyard fixture owns body-pair contact propagation.
      knockbackAreaMovement: (_world, id, delta) => {
        const target = liveTargets.find(target => target.id === id)
        if (target) target.position = { x: Math.fround(target.position.x + delta.x), y: Math.fround(target.position.y + delta.y) }
        return []
      },
      sceneryTargets: () => scenery,
    }
    const result = secondary.stepNativeSecondaryAbilities(state, world)
    state = result.state
    for (const actor of state.actors) seenKinds.add(actor.kind)
    if (sampleTicks.has(tick) || mode === 'live-equipment' && result.damage.length > 0) {
      const { state: ignored, ...outbox } = result
      samples.push({ tick, result: outbox, state, targetPositions:liveTargets.map(target=>[target.id,target.position]) })
    }
  }
  if (mode === 'live-equipment') {
    const epochDamages = [
      samples.filter(sample => sample.tick < 800),
      samples.filter(sample => sample.tick >= 800 && sample.tick < 1500),
      samples.filter(sample => sample.tick >= 1500),
    ].map(epoch => [...new Set(epoch.flatMap(sample => sample.result.damage.map(contact => contact.amount)))])
    if (epochDamages.some(epoch => epoch.length !== 1)
      || epochDamages[0][0] === epochDamages[1][0]
      || epochDamages[0][0] !== epochDamages[2][0]) {
      throw new Error('Leviathan must hit before, during and after the live equipment change')
    }
  }
  cases.push({ name: `${skillId}-${mode}`, skillId, mode, authority: initialAuthority, initial, targets, dampen, scenery, movement:'unobstructed-target-roots', samples })
}
for (const skillId of contract.NATIVE_SECONDARY_ABILITY_IDS) record(skillId, 'normal')
for (const skillId of [11, 12, 21, 27, 35, 41, 45, 72, 74, 76]) record(skillId, 'enhanced')
for (const skillId of [11, 21, 27, 35, 45, 76]) record(skillId, 'maximum')
for (const skillId of [23, 78, 79]) record(skillId, 'toggle-off')
record(23, 'overload')
record(23, 'ineligible-toggle')
record(11, 'world-switch')
record(35, 'effects')
record(54, 'shield-break')
record(54, 'shield-explode')
record(45, 'golem-death')
record(50, 'electric-trap')
record(11, 'live-equipment')
const feedback = []
for (const mode of ['flash', 'script-fires', 'mindblast', 'mindblast-gameover']) {
  for (const element of mode.startsWith('mindblast') ? ['air', 'earth', 'ether', 'fire', 'water'] : ['ether']) {
    const player = authority(allLearnedBook(), factors(false))
    player.input = { ...player.input, aim: { x: 100, y: 0 }, cast: { primary: false, quickbar: null } }
    const order = { actor: 10, transient: 20 }
    const registerWorldPainter = managerLane => ({ managerLane, registrationOrdinal: order[managerLane]++ })
    let state = secondary.createNativeSecondarySimulation(77)
    let response = null
    let result = null
    const fires = [
      { damage: 10, lifetimeTicks: 50, position: { x: 20, y: 0 }, radius: 100 },
      { damage: 0, lifetimeTicks: 100, position: { x: -100, y: 0 }, radius: 50 },
    ]
    if (mode === 'flash') {
      response = harmful.resolvePlayerFlashResponse({ flashChancePercent: 100, flashDurationTicks: 400 }, random.createNativeRng(15)).flash
      state = secondary.materializeNativePlayerFlashResponse(state, { ownerId: 'player', position: { x: 0, y: 0 }, response, targetIds: [7, 8], tick: 0, worldKey: player.worldKey })
      state = secondary.emitNativePlayerScreenFlash(state, { ownerId: 'player', position: { x: 0, y: 0 }, screenFlash: { red: 1, green: 0.5, blue: 0.25, decayPerTick: 0.1, pointAttenuated: true, alpha: 1 }, tick: 0, worldKey: player.worldKey })
    } else if (mode === 'script-fires') {
      state = secondary.spawnNativeScriptFires(state, 'player', player.worldKey, fires, registerWorldPainter)
    } else {
      const triggered = secondary.triggerNativePlayerMindblast(state, { element, level: 7, lightRegistration: { managerLane: 'actor', registrationOrdinal: 2 }, ownerId: 'player', position: { x: 0, y: 0 }, presentationScale: 4.5, registerWorldPainter, worldKey: player.worldKey })
      state = triggered.state
      result = { directDamage: triggered.directDamage, directRadius: triggered.directRadius }
    }
    const initial = state
    const samples = []
    for (let tick = 1; tick <= 250; tick++) {
      let outbox = null
      if (mode === 'mindblast-gameover') {
        state = secondary.stepNativeMindblastPresentation(state, tick)
      } else {
        const stepped = secondary.stepNativeSecondaryAbilities(state, { ...context(player, tick), registerWorldPainter,
          targetExists: (key, id) => key === 'boneyard:oracle' && targets.some(target => target.id === id),
          target: (_world, id) => targets.find(target => target.id === id) ?? null,
          targets: (_world, center, radius) => targets.filter(target => Math.hypot(target.position.x - center.x, target.position.y - center.y) <= radius + target.radius),
        })
        state = stepped.state
        const { state: ignored, ...remaining } = stepped
        outbox = remaining
      }
      if ([1, 2, 3, 10, 20, 25, 35, 36, 50, 100, 200, 230, 250].includes(tick)) samples.push({ tick, state, outbox })
    }
    feedback.push({ mode, element, authority: player, targets, dampen, scenery, fires, response, result, initial, samples })
  }
}
writeFixture('secondary-tick-goldens.json.zlib', 'tools/website-oracle/secondary-tick-goldens.mjs',
  'Current Website/frontend/src/game/core-kernels/native-secondary-abilities.ts',
  { sourceRevision, cases, seenKinds: [...seenKinds].sort(), feedback,
    liveEquipmentFactors: { plain:factors(false), equipped:factors(true) } })
