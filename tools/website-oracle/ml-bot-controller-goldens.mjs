#!/usr/bin/env node
// Drive the actual Source controller with deferred inference Promises and real Game APIs.
import { readFileSync } from 'node:fs'
import { execFileSync } from 'node:child_process'
import { resolve } from 'node:path'
import { fixturesDir, loadKernel, websiteDir, writeFixture } from './oracle.mjs'
const sourceRevision=execFileSync('git',['rev-parse','HEAD'],{cwd:websiteDir,encoding:'utf8'}).trim()

const game = await loadKernel('core-server/game-simulation.ts')
const saves = await loadKernel('save/game-save-document.ts')
const entities = await loadKernel('core-server/player-entity-store.ts')
const run = await loadKernel('core-kernels/game-run.ts')
const { DEFAULT_BONEYARD_CHOICE } = await loadKernel('host/boneyard-catalog.ts')
const { NATIVE_GENERATED_BONEYARDS } = await loadKernel('host/native-generated-boneyards.ts')
const { MlBotHostController } = await loadKernel('host/ml-bot-host-controller.ts')
const source = JSON.parse(readFileSync(resolve(fixturesDir, 'ml-bot-observer-goldens.json')))
const plain = value => JSON.parse(JSON.stringify(value, (_, item) => ArrayBuffer.isView(item) ? Array.from(item) : item))
const words = value => Array.from(new Uint32Array(value.buffer, value.byteOffset, value.length))
const idle = { aim: null, cast: { primary: false, quickbar: null }, movement: { x: 0, y: 0 }, viewportWidth: 1600, viewportHeight: 900 }
const character = { displayName: 'Policy Bot', element: 'fire', discipline: 'arcane' }
const action = { ability: 1, aim: 0, movement: 1, target: 1 }
const idleAction = { ability: 0, aim: 0, movement: 0, target: 0 }
const specs = [
  { name: 'entrance-before-offer-and-loadout-before-entrance', stockIndex: 0, operations: [
    { kind: 'poll' }, { kind: 'grant', amount: 500 }, { kind: 'poll' },
    { kind: 'finishRun' }, { kind: 'poll' }, { kind: 'poll' },
  ] },
  { name: 'captured-action-and-current-deadline', operations: [
    { kind: 'context', available: false }, { kind: 'poll' }, { kind: 'context', available: true },
    { kind: 'poll' }, { kind: 'poll' }, { kind: 'advance', count: 5, movement: { x: 1, y: 0 } },
    { kind: 'damage', amount: 1e9 }, { kind: 'complete', action }, { kind: 'poll' },
    { kind: 'advance', count: 9 }, { kind: 'poll' }, { kind: 'advance', count: 1 },
    { kind: 'poll' },
  ] },
  { name: 'ordinary-current-completion-cadence', operations: [
    { kind: 'poll' }, { kind: 'advance', count: 9 }, { kind: 'poll' },
    { kind: 'complete', action }, { kind: 'advance', count: 9 }, { kind: 'poll' },
    { kind: 'advance', count: 1 }, { kind: 'poll' },
    { kind: 'complete', action: idleAction }, { kind: 'poll' },
  ] },
  { name: 'pending-world-context-and-worker-failure', operations: [
    { kind: 'poll' }, { kind: 'switch', sourceIndex: 1 }, { kind: 'poll' },
    { kind: 'complete', action }, { kind: 'poll' }, { kind: 'context', available: false },
    { kind: 'complete', action }, { kind: 'context', available: true }, { kind: 'poll' },
    { kind: 'complete', error: 'actual worker rejection' }, { kind: 'poll' },
  ] },
  { name: 'illegal-completion-fails-once', operations: [
    { kind: 'poll' }, { kind: 'complete', action: { ...idleAction, ability: 21, target: 1 } },
    { kind: 'poll' }, { kind: 'hub' }, { kind: 'poll' },
  ] },
  { name: 'learned-offer-generations', operations: [
    { kind: 'grant', amount: 500 }, { kind: 'poll' }, { kind: 'poll' },
    { kind: 'chooseFirst' }, { kind: 'complete', selectedOption: 0 }, { kind: 'poll' },
    { kind: 'complete', selectedOption: 999 }, { kind: 'poll' },
    { kind: 'complete', selectedOption: 1 }, { kind: 'poll' }, { kind: 'applyIntent' },
    { kind: 'poll' }, { kind: 'chooseFirst' }, { kind: 'complete', selectedOption: 0 }, { kind: 'poll' },
  ] },
  { name: 'scripted-choice-generation', choiceMode: 'scripted', operations: [
    { kind: 'grant', amount: 500 }, { kind: 'poll' }, { kind: 'poll' },
    { kind: 'applyIntent' }, { kind: 'poll' }, { kind: 'poll' },
    { kind: 'applyIntent' }, { kind: 'poll' },
  ] },
  { name: 'post-run-loadout-and-world-reset', operations: [
    { kind: 'finishRun' }, { kind: 'poll' }, { kind: 'poll' }, { kind: 'hub' },
    { kind: 'poll' }, { kind: 'switch', sourceIndex: 1 }, { kind: 'finishRun' },
    { kind: 'poll' }, { kind: 'poll' },
  ] },
]
const cases = []
for (const spec of specs) {
  let state = saves.restoreGameSaveDocument(JSON.stringify(source.cases[0].document)).state
  let initialDocument = null
  if (spec.stockIndex !== undefined) {
    const loaded = { ...NATIVE_GENERATED_BONEYARDS[spec.stockIndex], seed: 'controller-entrance', runId: 'controller-entrance',
      choice: DEFAULT_BONEYARD_CHOICE }
    state = game.enterBoneyardWorld(game.createGameSimulation({ local: character }), loaded)
    initialDocument = JSON.parse(saves.createGameSaveDocument({ state, playerId: 'local', loadedBoneyard: loaded,
      integrity: 'global-clean', mods: [], modState: {} }))
    state = saves.restoreGameSaveDocument(JSON.stringify(initialDocument)).state
  }
  let available = true
  let nextId = 1
  let pending = null
  let lastIntent = null
  const outputs = []
  const activeInputs = {}
  const controllers = { local: 'bot' }
  const inference = {
    choiceMode: spec.choiceMode ?? 'learned',
    infer(observation, plan) {
      outputs.push({ request: { kind: 'actions', id: nextId++, observationBits: words(observation), plan: plain(plan) } })
      return new Promise((resolve, reject) => { pending = { resolve, reject } })
    },
    inferChoice(observation, descriptors, mask) {
      outputs.push({ request: { kind: 'choice', id: nextId++, observationBits: words(observation), descriptorBits: words(descriptors), mask: Array.from(mask) } })
      return new Promise((resolve, reject) => { pending = { resolve, reject } })
    },
  }
  const controller = new MlBotHostController({
    context: () => available ? { state, activeInputs, controllers } : null,
    dispatch: intent => { lastIntent = intent; outputs.push({ intent: plain(intent) }) },
    fail: error => outputs.push({ error: error.message }),
  }, character, inference, 'local')
  const records = []
  for (const operation of spec.operations) {
    switch (operation.kind) {
      case 'poll':
        try { controller.tick() } catch (error) { outputs.push({ error: error.message }) }
        break
      case 'context': available = operation.available; break
      case 'switch': state = saves.restoreGameSaveDocument(JSON.stringify(source.cases[operation.sourceIndex].document)).state; break
      case 'hub': state = game.returnGameSimulationToHub(state); break
      case 'advance':
        for (let index = 0; index < operation.count; index += 1) {
          state = game.stepGameSimulationTick(state, { local: { ...idle, movement: operation.movement ?? idle.movement } })
        }
        break
      case 'damage': state = { ...state, playerEntities: entities.damagePlayerEntity(state.playerEntities, 'local', operation.amount, state.tick) }; break
      case 'grant': state = game.grantGameSimulationPlayerExperience(state, 'local', operation.amount); break
      case 'chooseFirst': {
        const offer = game.getPlayerProgression(state, 'local').pendingOffer
        const next = game.selectGameSimulationPlayerSkill(state, 'local', {
          choiceIndex: 0, offerSequence: offer.sequence, skillId: offer.options[0].skillId,
        })
        if (!next) throw new Error(`${spec.name}: first skill rejected`)
        state = next
        break
      }
      case 'applyIntent': {
        if (lastIntent?.kind !== 'select-skill') throw new Error(`${spec.name}: no skill intent`)
        const next = game.selectGameSimulationPlayerSkill(state, 'local', lastIntent)
        if (!next) throw new Error(`${spec.name}: skill intent rejected`)
        state = next
        break
      }
      case 'finishRun': {
        for (let count = 0; state.run.phase !== 'loadout'; count += 1) {
          if (count > 1500) throw new Error('run lifecycle did not reach loadout')
          state = { ...state, run: run.stepGameRunLifecycle(state.run, new Set()) }
        }
        break
      }
      case 'complete': {
        if (!pending) throw new Error(`${spec.name}: no pending request at operation ${records.length}; phase ${state.run.phase}, tick ${state.tick}`)
        const request = pending
        pending = null
        if (operation.error) request.reject(new Error(operation.error))
        else if (operation.action) request.resolve({ actions: operation.action, logProbability: 0, value: 0 })
        else request.resolve({ selectedOption: operation.selectedOption, logProbability: 0, value: 0 })
        await new Promise(resolve => setImmediate(resolve))
        break
      }
      default: throw new Error(`unknown operation ${operation.kind}`)
    }
    records.push({ operation, tick: state.tick, phase: state.run.phase, outputs: outputs.splice(0) })
  }
  cases.push({ name: spec.name, choiceMode: spec.choiceMode ?? 'learned', character, ...(initialDocument ? { initialDocument } : {}), records })
}
writeFixture('ml-bot-controller-goldens.json', 'tools/website-oracle/ml-bot-controller-goldens.mjs',
  'Website host/ml-bot-host-controller.ts with real deferred Promises, public Game transactions and run lifecycle', { sourceRevision, cases })
console.log(JSON.stringify({ cases: cases.length, operations: cases.reduce((sum,row)=>sum+row.records.length,0) }))
