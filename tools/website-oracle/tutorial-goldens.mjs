#!/usr/bin/env node
// Retain the complete native Tutorial state machine and its authored tables.
import { loadKernel, writeFixture } from './oracle.mjs'

const tutorial = await loadKernel('core-kernels/native-tutorial.ts')
const position = { x: 1025, y: 2070.0703125 }
const seed = 'tutorial-oracle'
const initial = () => tutorial.createNativeTutorialState(position, 3, seed)
const baseInput = {
  acidRainCastSequence: 0, acidRainLastSkillId: null, cameraLockSafetyClear: true,
  currentHealth: 100, enemyCount: 0, groundSackCount: 0, hasTopLevelNonPotionItem: false,
  healthPotionCount: 0, level: 1, levelUpPending: false, maximumHealth: 100,
  playerActionIdle: true, playerPosition: position, primaryCastSequence: 3,
  solomonPhase: null, solomonRunEventId: 0, tick: 1,
}
const afterIntro = { introActive: false, introBlend: 1, introDelayTicksRemaining: 0,
  introFade: 0, introMovementTicksRemaining: 250 }
const bindings = { inventory: 'I', moveDown: 'S', moveLeft: 'A', moveRight: 'D',
  moveUp: 'W', potion: 'Q', secondary: 'SPACE', skills: 'K' }
function presentation(state) {
  return {
    desktop: tutorial.nativeTutorialPresentation(state, bindings, 'desktop'),
    mobile: tutorial.nativeTutorialPresentation(state, bindings, 'mobile'),
    baselines: tutorial.nativeTutorialInstructionBaselines(state.stage, 900),
    forcedVelocity: tutorial.nativeTutorialForcedVelocity(state),
    hostilePaused: tutorial.nativeTutorialHostileScenePaused(state),
    camera: tutorial.nativeTutorialCameraBounds(state),
  }
}
const steps = []
function step(name, patch = {}, inputPatch = {}) {
  const source = { ...initial(), ...afterIntro, ...patch }
  const input = { ...baseInput, ...inputPatch }
  const result = tutorial.stepNativeTutorial(source, input)
  steps.push({ name, patch, input, expected: result, presentation: presentation(result.state) })
}
for (const stage of tutorial.NATIVE_TUTORIAL_STAGES) {
  step(`stage-${stage}-retained`, { stage }, { enemyCount: 5, healthPotionCount: 1, groundSackCount: 1 })
  step(`stage-${stage}-empty`, { stage })
}
step('first-dialogue', { stageTicks: 50 })
step('move-to-solomon', {}, { playerPosition: { x: 1400, y: 1000 } })
step('begin-first-wave', { stage: 1 }, { solomonRunEventId: 1 })
step('primary-cast-acknowledged', { stage: 2 }, { primaryCastSequence: 4, enemyCount: 8 })
step('acid-rain-opens-item-wave', { stage: 5 }, { acidRainCastSequence: 1, acidRainLastSkillId: 72 })
step('already-collected-amulet', { stage: 7 }, { hasTopLevelNonPotionItem: true })
step('collect-amulet', { stage: 8 }, { hasTopLevelNonPotionItem: true })
step('inventory-opened', { stage: 9, inventoryOpened: true })
step('inventory-closed', { stage: 10, inventorySeen: true })
step('grant-level-experience', { stage: 11, stageTicks: 100 })
step('level-pending-keeps-stage', { stage: 11 }, { level: 2, levelUpPending: true })
step('level-action-keeps-stage', { stage: 11 }, { level: 2, playerActionIdle: false })
step('level-complete', { stage: 11 }, { level: 2 })
step('skills-opened', { stage: 12, skillsOpened: true })
step('skills-closed', { stage: 13, skillsSeen: true })
step('archer-wave-present', { stage: 15 }, { enemyCount: 3 })
step('wounded-under-four-enemies', { stage: 14 }, { enemyCount: 3, currentHealth: 50 })
step('survival-release', { stage: 18 }, { tick: 1000 })
step('tutorial-complete', { stage: 19 }, { enemyCount: 6 })
step('solomon-speaking-queue', { stage: 3 }, { solomonPhase: 'speaking' })
step('solomon-retreat-queue', { stage: 3 }, { solomonPhase: 'retreat-accelerating' })
step('camera-lock-start', { stage: 3 }, { playerPosition: { x: 1000, y: 500 } })
step('camera-lock-invalidated', { stage: 3, cameraLockTriggered: true, cameraLockAgeTicks: 123,
  cameraLockTicksRemaining: 177 }, { cameraLockSafetyClear: false })

const intro = []
let introState = initial()
for (let tick = 0; tick < 510; tick += 1) {
  const result = tutorial.stepNativeTutorial(introState, { ...baseInput, tick })
  introState = result.state
  if (tick < 30 || tick % 10 === 0 || !introState.introActive) {
    intro.push({ tick, state: introState, presentation: presentation(introState) })
  }
  if (!introState.introActive) break
}

const waves = []
for (let waveOrdinal = 1; waveOrdinal <= 6; waveOrdinal += 1) {
  let state = { ...initial(), ...afterIntro, active: false, stage: 19,
    waveOrdinal, survivalEnabled: waveOrdinal === 6 }
  const frames = []
  for (let tick = 1; tick <= 1600; tick += 1) {
    const input = { ...baseInput, tick, playerPosition: { x: 1000, y: 500 },
      enemyCount: tick < 600 ? 20 : tick < 900 ? 150 : 0, level: tick < 1000 ? 1 : 4 }
    const result = tutorial.stepNativeTutorial(state, input)
    state = result.state
    if (result.spawnIntents.length > 0 || tick === 1 || tick % 100 === 0) {
      frames.push({ tick, input, expected: result })
    }
  }
  waves.push({ waveOrdinal, frames })
}

const surfaces = []
for (const stage of tutorial.NATIVE_TUTORIAL_STAGES) {
  for (const action of tutorial.NATIVE_TUTORIAL_SURFACE_ACTIONS) {
    const source = { ...initial(), ...afterIntro, stage, inventorySeen: true, skillsSeen: true }
    const state = tutorial.applyNativeTutorialSurfaceAction(source, action)
    surfaces.push({ stage, action, state, presentation: presentation(state) })
  }
}
const acknowledgements = []
for (const introActive of [false, true]) for (const stage of [0, 1]) {
  for (const movementEpochActive of [false, true]) for (const userMovementRequested of [false, true]) {
    const source = { ...initial(), ...afterIntro, introActive, stage }
    const input = { movementEpochActive, userMovementRequested }
    acknowledgements.push({ introActive, stage, input,
      state: tutorial.acknowledgeNativeTutorialMovementInstruction(source, input) })
  }
}
const narration = []
let narrationState = { ...initial(), ...afterIntro, active: false,
  narration: { current: null, nextEventId: 1, pending: [...tutorial.NATIVE_TUTORIAL_CUES], ticksRemaining: 0 } }
for (let tick = 1; tick < 15000; tick += 1) {
  const previous = narrationState.narration
  narrationState = tutorial.stepNativeTutorial(narrationState, { ...baseInput, tick }).state
  if (narrationState.narration.current?.eventId !== previous.current?.eventId
    || narrationState.narration.ticksRemaining <= 1) narration.push({ tick, state: narrationState.narration })
  if (narrationState.narration.current === null && narrationState.narration.pending.length === 0) break
}
const cameras = Array.from({ length: 930 }, (_, index) => {
  const age = index / 2
  const source = { ...initial(), cameraLockTriggered: true, cameraLockAgeTicks: age }
  return { age, bounds: tutorial.nativeTutorialCameraBounds(source) }
})
const domains = []
for (const x of [-20, 0, 300, 900, 1172, 1500, 2000, 2060]) {
  for (const y of [0, 100, 824.91796875, 849.91796875, 1550, 1605, 1620, 1700]) {
    for (const radius of [0, 15, 25, 50]) {
      const point = { x, y }
      domains.push({ point, radius, camera: tutorial.nativeTutorialEnemyCameraPositionIsAllowed(point, radius),
        spawn: tutorial.nativeTutorialEnemySpawnPositionIsAllowed(point, radius) })
    }
  }
}
writeFixture('tutorial-goldens.json', 'tools/website-oracle/tutorial-goldens.mjs',
  'Website/frontend/src/game/core-kernels/native-tutorial.ts', {
    initial: initial(), seed, position, baseInput, bindings, steps, intro, waves,
    surfaces, acknowledgements, narration, cameras, domains,
    tables: { recipes: tutorial.NATIVE_TUTORIAL_MONSTER_RECIPES, groups: tutorial.NATIVE_TUTORIAL_UID_GROUPS,
      batches: tutorial.NATIVE_TUTORIAL_WAVE_BATCHES, cues: tutorial.NATIVE_TUTORIAL_CUE_DEFINITIONS,
      fires: tutorial.NATIVE_TUTORIAL_FIRES, amulet: tutorial.nativeTutorialAmuletItem(),
      potion: tutorial.nativeTutorialHealthPotionItem(), dialogueTicks: tutorial.nativeTutorialDialogueTicks() },
  })
