#!/usr/bin/env node
// The host's queue functions are private. Select their unchanged AST nodes and the actual
// client-input branch; only socket/query callbacks are supplied by the fixture boundary.
import { createHash } from 'node:crypto'
import { readFileSync, mkdirSync, writeFileSync } from 'node:fs'
import { createRequire } from 'node:module'
import { dirname, resolve } from 'node:path'
import { gameDir, loadKernel, root, websiteDir } from './oracle.mjs'

const require = createRequire(resolve(websiteDir, 'frontend/package.json'))
const ts = require('typescript')
const source = readFileSync(resolve(gameDir, 'host/game-host.ts'), 'utf8')
const tree = ts.createSourceFile('game-host.ts', source, ts.ScriptTarget.Latest, true)
const functions = new Map()
let inputBranch
function visit(node) {
  if (ts.isFunctionDeclaration(node) && node.name) functions.set(node.name.text, node.getText(tree))
  if (ts.isIfStatement(node) && node.expression.getText(tree) === "message.type === 'client-input'") {
    if (inputBranch) throw new Error('source has multiple client-input branches')
    inputBranch = node.getText(tree)
  }
  ts.forEachChild(node, visit)
}
visit(tree)
if (!inputBranch) throw new Error('source client-input branch is missing')
const helpers = ['applyQueuedInput', 'newestQueuedInput', 'sameCast'].map(name => {
  const body = functions.get(name)
  if (!body) throw new Error(`source ${name} is missing`)
  return body
}).join('\n')
const { GAME_TICK_RATE } = await loadKernel('core-server/game-simulation.ts')
const { createIdlePlayerCharacterInput } = await loadKernel('core-kernels/player-character.ts')
const { decodePlayerCharacterInput } = await loadKernel('protocol/codecs/input.ts')
const program = `
const { GAME_TICK_RATE, createIdlePlayerCharacterInput } = bindings;
${helpers}
function accept(client, message, context, output) {
  const socket = {};
  const stateForPlayer = () => ({ tick: context.tick, world: { kind: context.hub ? 'hub' : 'boneyard' }, levelUpBarrier: context.barrier ? {} : null });
  const gameplayPauseForPlayer = () => context.pause ? {} : null;
  const gameplayResumeGraceForClient = () => context.grace ? {} : null;
  const getPlayerProgression = () => ({ pendingOffer: context.offer ? {} : null });
  const broadcastSnapshot = () => output.push('snapshot');
  const disconnect = (_socket, code, reason) => output.push({ code, reason });
  ${inputBranch}
}
return { accept, applyQueuedInput };
`
const compiled = ts.transpileModule(program, { compilerOptions: { target: ts.ScriptTarget.ES2022 } }).outputText
const kernel = new Function('bindings', compiled)({ GAME_TICK_RATE, createIdlePlayerCharacterInput })
function input(primary = false, quickbar = null, x = 0) {
  return { ...createIdlePlayerCharacterInput(), cast: { primary, quickbar }, movement: { x, y: 0 } }
}
function record(client) {
  return {
    lastReceivedSequence: client.lastReceivedSequence,
    acknowledgedSequence: client.acknowledgedSequence,
    activeInput: structuredClone(client.activeInput),
    queuedInputs: [...client.queuedInputs.values()].map(value => structuredClone(value)),
  }
}
function scenario(name, actions) {
  const client = { playerId: 'local', lastReceivedSequence: 0, acknowledgedSequence: 0,
    activeInput: createIdlePlayerCharacterInput(), queuedInputs: new Map(), hubActivity: null }
  return { name, steps: actions.map(action => {
    const output = []
    if (action.kind === 'input') {
      client.hubActivity = action.context.activity ? 'inventory' : null
      kernel.accept(client, { type: 'client-input', sequence: action.sequence, targetTick: action.targetTick, input: action.input }, action.context, output)
    } else kernel.applyQueuedInput(client, action.tick)
    return { action, output, expected: record(client) }
  }) }
}
const send = (sequence, targetTick, value, context = {}) => ({ kind: 'input', sequence, targetTick, input: value, context: { tick: 10, ...context } })
const tick = value => ({ kind: 'tick', tick: value })
const scenarios = [
  scenario('same-arrival-cast-edges-and-movement-coalescing', [
    send(1, 0, input(true)), send(2, 0, input(false)), send(3, 0, input(true, 2)),
    send(4, 0, input(true, 2, 1)), tick(11), tick(12), tick(13),
    send(4, 999, input(false)), tick(14),
  ]),
  scenario('future-bound-and-rejected-queue-do-not-advance-received-sequence', [
    send(1, 210, input()), send(2, 0, input(true)), send(2, 211, input()),
    send(2, 0, input()), tick(209), tick(210), tick(211),
  ]),
]
for (const blocker of ['pause', 'grace', 'barrier', 'offer', 'activity']) {
  scenarios.push(scenario(`neutralize-${blocker}`, [
    send(1, 11, input(true, 1, 1)),
    send(2, 999, input(true, 3, -1), { [blocker]: true, hub: blocker === 'activity' }),
    tick(11), send(3, 0, input(false), { tick: 11 }), tick(12),
  ]))
}
scenarios.push(scenario('hub-activity-does-not-block-boneyard-input', [
  send(1, 0, input(true), { activity: true }), tick(11),
]))
const validations = []
for (const x of [0, 1, 1.001, 1.001000001, -1.001, -1.002]) {
  validations.push({ ...input(), movement: { x, y: 0 } })
}
for (const value of [0, 1, 32768, 32768.001]) {
  validations.push({ ...input(), viewportWidth: value })
  validations.push({ ...input(), viewportHeight: value })
}
// Retain the unchanged Source validator over the common valid/invalid domain;
// current three-bank acceptance is exercised through the native input interfaces.
for (const slot of [0, 7, 24]) validations.push(input(false, slot))
const validationRecords = validations.map(value => {
  let accepted = true
  try { decodePlayerCharacterInput(value, 'input') } catch { accepted = false }
  return { input: value, accepted }
})
const path = resolve(root, 'crates/launcher/tests/fixtures/website/host-input-goldens.json')
mkdirSync(dirname(path), { recursive: true })
writeFileSync(path, `${JSON.stringify({ source: 'Website/frontend/src/game/host/game-host.ts',
  sourceSha256: createHash('sha256').update(source).digest('hex'), generator: 'tools/website-oracle/host-input-goldens.mjs', scenarios, validations: validationRecords })}\n`)
console.log(`wrote ${scenarios.length} host input scenarios / ${scenarios.reduce((sum, row) => sum + row.steps.length, 0)} accepted checkpoints`)
