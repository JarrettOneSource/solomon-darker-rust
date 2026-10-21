#!/usr/bin/env node
// Extract the unchanged public sendInput method and its equality helpers. Transport and
// presentation callbacks are the fixture boundary; sequencing runs in the actual source.
import { createHash } from 'node:crypto'
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import { createRequire } from 'node:module'
import { dirname, resolve } from 'node:path'
import { gameDir, loadKernel, root, websiteDir } from './oracle.mjs'

const require = createRequire(resolve(websiteDir, 'frontend/package.json'))
const ts = require('typescript')
const source = readFileSync(resolve(gameDir, 'client/game-client-session.ts'), 'utf8')
const tree = ts.createSourceFile('game-client-session.ts', source, ts.ScriptTarget.Latest, true)
const functions = new Map()
let sendInput
function visit(node) {
  if (ts.isFunctionDeclaration(node) && node.name) functions.set(node.name.text, node.getText(tree))
  if (ts.isMethodDeclaration(node) && node.name.getText(tree) === 'sendInput') {
    if (sendInput) throw new Error('multiple public sendInput implementations')
    sendInput = node.getText(tree)
  }
  ts.forEachChild(node, visit)
}
visit(tree)
if (!sendInput) throw new Error('public sendInput implementation missing')
const helpers = ['copyInput', 'sameInput', 'sameCast'].map(name => {
  const body = functions.get(name)
  if (!body) throw new Error(`source ${name} missing`)
  return body
}).join('\n')
const { createIdlePlayerCharacterInput } = await loadKernel('core-kernels/player-character.ts')
const program = `
  const STOPPED_INPUT = createIdlePlayerCharacterInput();
  ${helpers}
  let welcome = { playerId: 'local' }, destroyed = false;
  let snapshot, gameplayPause = null, gameplayResumeGrace = null;
  let currentInput = copyInput(STOPPED_INPUT), sentInput = copyInput(STOPPED_INPUT);
  let sequence = 0, pendingInputs = [], output = [];
  const advanceLocalHubPresentation = () => {}, now = () => 0;
  const encodeGameMessage = message => message;
  const options = { transport: { send: message => output.push(structuredClone(message)) } };
  const client = { ${sendInput} };
  return action => {
    output = [];
    if (action.kind === 'send') {
      snapshot = { tick: action.tick, levelUpBarrier: action.barrier ? {} : null,
        players: { local: { progression: { pendingOffer: action.offer ? {} : null,
          lifeState: action.dead ? 'dead' : 'alive' } } },
        run: { phase: action.gameOver ? 'game-over' : 'active' } };
      gameplayPause = action.pause ? {} : null;
      gameplayResumeGrace = action.grace ? {} : null;
      try { client.sendInput(action.input); } catch (error) { output.push({ error: error.message }); }
    } else if (action.kind === 'ack') {
      pendingInputs = pendingInputs.filter(entry => entry.sequence > action.sequence);
    } else if (action.kind === 'reset') {
      currentInput = copyInput(STOPPED_INPUT);
      sentInput = copyInput(STOPPED_INPUT);
      pendingInputs = [];
    } else throw new Error('unknown fixture action');
    return { output, sequence, pendingInputs: structuredClone(pendingInputs) };
  };
`
const compiled = ts.transpileModule(program, { compilerOptions: { target: ts.ScriptTarget.ES2022 } }).outputText
const createSequence = new Function('createIdlePlayerCharacterInput', compiled)
const input = (primary = false, quickbar = null, movement = { x: 0, y: 0 }) => ({
  ...createIdlePlayerCharacterInput(), cast: { primary, quickbar }, movement,
})
const send = (value, context = {}) => ({ kind: 'send', input: value, tick: 100, ...context })
const scenarios = []
function scenario(name, actions) {
  const step = createSequence(createIdlePlayerCharacterInput)
  scenarios.push({ name, steps: actions.map(action => ({ action, expected: step(action) })) })
}
scenario('rapid-cast-edges-retain-ticks-movement-replaces-same-tick', [
  send(input()), send(input(true)), send(input(false)), send(input(true, 2)),
  send(input(true, 2, { x: 1, y: 0 })), send(input(true, 2, { x: 0, y: 1 })),
  send(input(false, 2, { x: 0, y: 1 })),
  { kind: 'ack', sequence: 3 }, send(input(false, null)),
  { kind: 'ack', sequence: 7 }, send(input(true, null), { tick: 112 }),
])
scenario('normalization-aim-viewport-and-duplicate-suppression', [
  send(input(false, null, { x: 3, y: 4 })),
  send(input(false, null, { x: 0.6, y: 0.8 })),
  send({ ...input(), aim: { x: 20, y: 30 }, viewportWidth: 390, viewportHeight: 844 }),
  send({ ...input(), aim: { x: 20, y: 30 }, viewportWidth: 390, viewportHeight: 844 }),
  send({ ...input(), aim: { x: 21, y: 30 }, viewportWidth: 390, viewportHeight: 844 }),
  send(input(false, 8)), send({ ...input(), viewportWidth: 0 }), send(input(true)),
])
for (const blocker of ['pause', 'grace', 'barrier', 'offer', 'dead', 'gameOver']) {
  scenario(`stopped-${blocker}-and-authority-reset`, [
    send(input(true, 1, { x: 1, y: 0 })),
    send(input(true, 1, { x: 1, y: 0 }), { [blocker]: true }),
    { kind: 'reset' }, send(input()), send(input(true), { tick: 140 }),
  ])
}
const path = resolve(root, 'crates/protocol/tests/fixtures/website/client-input-goldens.json')
mkdirSync(dirname(path), { recursive: true })
writeFileSync(path, `${JSON.stringify({ source: 'Website/frontend/src/game/client/game-client-session.ts',
  sourceSha256: createHash('sha256').update(source).digest('hex'),
  generator: 'tools/website-oracle/client-input-goldens.mjs', scenarios })}\n`)
console.log(`wrote ${scenarios.length} client input scenarios / ${scenarios.reduce((total, item) => total + item.steps.length, 0)} source checkpoints`)
