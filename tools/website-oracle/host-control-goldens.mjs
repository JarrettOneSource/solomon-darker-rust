#!/usr/bin/env node
// Private host functions are selected unchanged. Only network/log callbacks and the
// monotonic clock are supplied by this recording boundary; Website remains untouched.
import { createHash } from 'node:crypto'
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import { createRequire } from 'node:module'
import { dirname, resolve } from 'node:path'
import { gameDir, loadKernel, root, websiteDir } from './oracle.mjs'

const require = createRequire(resolve(websiteDir, 'frontend/package.json'))
const ts = require('typescript')
const source = readFileSync(resolve(gameDir, 'host/game-host.ts'), 'utf8')
const tree = ts.createSourceFile('game-host.ts', source, ts.ScriptTarget.Latest, true)
const functions = new Map()
let pauseBranch
function visit(node) {
  if (ts.isFunctionDeclaration(node) && node.name) functions.set(node.name.text, node.getText(tree))
  if (ts.isIfStatement(node) && node.expression.getText(tree) === "message.type === 'client-gameplay-pause'") {
    if (pauseBranch) throw new Error('source has multiple pause branches')
    pauseBranch = node.getText(tree)
  }
  ts.forEachChild(node, visit)
}
visit(tree)
if (!pauseBranch) throw new Error('source pause branch is missing')
const names = [
  'armBoneyardRendererReadiness', 'markBoneyardRendererReady', 'boneyardRenderersReady',
  'gameplayPauseForPlayer', 'projectGameplayResumeGrace', 'nextResumeGraceSequence',
  'beginRunLoadingResumeGrace', 'beginSurfaceResumeGrace', 'connectedMaterializedHumanPlayerIds',
  'acknowledgeResumeGraceReady', 'maybeStartGameplayResumeGrace',
  'removeGameplayResumeGraceParticipant', 'expireGameplayResumeGraces',
  'gameplayPauseRecord', 'gameplayResumeGraceRecord', 'setGameplayResumeGrace',
  'stopResumeGraceInputs', 'setGameplayPauseForPlayer', 'releaseGameplayPause',
]
const helpers = names.map(name => {
  const body = functions.get(name)
  if (!body) throw new Error(`source ${name} is missing`)
  return body
}).join('\n')
const { gameplayResumeGraceReasonForPauseSource } = await loadKernel('gameplay-resume-grace.ts')
const { GAMEPLAY_RESUME_GRACE_DURATION_MS } = await loadKernel('protocol/game-protocol-contract.ts')
const program = `
const { gameplayResumeGraceReasonForPauseSource, GAMEPLAY_RESUME_GRACE_DURATION_MS } = bindings;
return function create() {
  let now = 0;
  const performance = { now: () => now };
  const sharedWorlds = null;
  let gameplayPause = null;
  let gameplayResumeGrace = null;
  let nextGameplayResumeGraceSequence = 1;
  const boneyardRendererReadiness = new Map();
  const state = { tick: 10, world: { kind: 'boneyard' }, run: { phase: 'active', runId: 'run-a' },
    levelUpBarrier: null, playerEntities: { identities: [{ playerId: 'a' }, { playerId: 'b' }] } };
  const clients = new Map(['a','b'].map(id => [id, { playerId: id, displayName: id.toUpperCase(), partyRejoinSlot: null }]));
  const options = {};
  const stateForPlayer = () => state;
  const gameplayResumeGraceForPlayer = () => projectGameplayResumeGrace(gameplayResumeGrace);
  const broadcast = () => {};
  const broadcastSnapshot = () => {};
  const broadcastGameplayPause = () => {};
  const broadcastGameplayResumeGrace = () => {};
  const stopAllClientInputs = () => {};
  const stopWorldClientInputs = () => {};
  const resetNextTickDeadline = () => {};
  const logGameplayResumeGrace = () => {};
  const logGameServerEvent = () => {};
  const logDetails = value => value;
  ${helpers}
  function pause(client, message) { ${pauseBranch} }
  function advance() {
    boneyardRenderersReady(state.run.runId);
    maybeStartGameplayResumeGrace('a');
    expireGameplayResumeGraces(now);
  }
  return { step(action) {
    if (action.now !== undefined) now = action.now;
    switch(action.kind) {
      case 'arm': armBoneyardRendererReadiness(state); break;
      case 'loading': beginRunLoadingResumeGrace(action.player, action.reason, null, true, action.required ?? []); break;
      case 'renderer': markBoneyardRendererReady(action.player, action.run); break;
      case 'resume_ready': acknowledgeResumeGraceReady(clients.get(action.player), action.sequence); break;
      case 'advance': advance(); break;
      case 'pause': pause(clients.get(action.player), {type:'client-gameplay-pause', paused:true, source:action.source}); break;
      case 'unpause': pause(clients.get(action.player), {type:'client-gameplay-pause', paused:false, source:'pause-menu'}); break;
      case 'surface': beginSurfaceResumeGrace(action.player, action.reason); break;
      case 'barrier': state.levelUpBarrier = action.enabled ? {} : null; break;
      case 'disconnect':
        clients.delete(action.player);
        state.playerEntities.identities = state.playerEntities.identities.filter(row => row.playerId !== action.player);
        removeGameplayResumeGraceParticipant(action.player, null);
        if (gameplayPause?.ownerPlayerId === action.player) releaseGameplayPause('owner-disconnected', action.player);
        advance();
        break;
      case 'rejoin':
        clients.set(action.player, {playerId:action.player, displayName:action.player.toUpperCase(), partyRejoinSlot:null});
        state.playerEntities.identities.push({playerId:action.player});
        break;
      case 'sequence': nextGameplayResumeGraceSequence = action.value; break;
      default: throw new Error('unrecorded action '+action.kind);
    }
    return { pause: structuredClone(gameplayPause), grace: projectGameplayResumeGrace(gameplayResumeGrace),
      loadingRunId: boneyardRendererReadiness.size ? state.run.runId : null,
      blocked: gameplayPause !== null || gameplayResumeGrace !== null || boneyardRendererReadiness.size !== 0 };
  }};
};`
const compiled = ts.transpileModule(program, { compilerOptions: { target: ts.ScriptTarget.ES2022 } }).outputText
const create = new Function('bindings', compiled)({ gameplayResumeGraceReasonForPauseSource, GAMEPLAY_RESUME_GRACE_DURATION_MS })
const scenario = (name, actions) => {
  const kernel = create()
  return { name, steps: actions.map(action => ({ action, expected: kernel.step(action) })) }
}
const scenarios = [
  scenario('all-renderers-and-current-resume-acks-gate-the-two-second-countdown', [
    {kind:'arm'}, {kind:'loading',player:'a',reason:'game-started'},
    {kind:'renderer',player:'a',run:'old-run'}, {kind:'resume_ready',player:'a',sequence:0},
    {kind:'renderer',player:'a',run:'run-a'}, {kind:'resume_ready',player:'a',sequence:1},
    {kind:'advance',now:100}, {kind:'renderer',player:'b',run:'run-a'},
    {kind:'resume_ready',player:'b',sequence:1}, {kind:'advance',now:500},
    {kind:'advance',now:2499.1}, {kind:'advance',now:2500},
  ]),
  scenario('only-pause-owner-can-change-or-release-and-inventory-resumes-with-grace', [
    {kind:'pause',player:'a',source:'pause-menu'}, {kind:'pause',player:'b',source:'inventory'},
    {kind:'unpause',player:'b'}, {kind:'pause',player:'a',source:'inventory'},
    {kind:'unpause',player:'a',now:80}, {kind:'pause',player:'b',source:'pause-menu'},
    {kind:'advance',now:2080},
  ]),
  scenario('selector-close-and-owner-disconnect-do-not-add-a-countdown', [
    {kind:'pause',player:'a',source:'skill-selector'}, {kind:'unpause',player:'a'},
    {kind:'pause',player:'b',source:'skill-book'}, {kind:'disconnect',player:'b'},
  ]),
  scenario('picker-close-waits-for-its-current-ack-and-barrier-then-releases-immediately', [
    {kind:'barrier',enabled:true}, {kind:'pause',player:'a',source:'inventory'},
    {kind:'surface',player:'a',reason:'skill-picker-closed'},
    {kind:'resume_ready',player:'b',sequence:1}, {kind:'resume_ready',player:'a',sequence:99},
    {kind:'resume_ready',player:'a',sequence:1}, {kind:'advance',now:100},
    {kind:'barrier',enabled:false}, {kind:'advance',now:101},
  ]),
  scenario('ordinary-disconnect-removes-waiter-and-renderer', [
    {kind:'arm'}, {kind:'loading',player:'a',reason:'game-rejoined'},
    {kind:'renderer',player:'a',run:'run-a'}, {kind:'resume_ready',player:'a',sequence:1},
    {kind:'disconnect',player:'b',now:40}, {kind:'advance',now:2040},
  ]),
  scenario('required-disconnected-living-wizard-keeps-party-rejoin-hold', [
    {kind:'loading',player:'b',reason:'party-rejoin-wait',required:['b']},
    {kind:'disconnect',player:'b'}, {kind:'resume_ready',player:'a',sequence:1},
    {kind:'advance',now:10000}, {kind:'rejoin',player:'b'},
    {kind:'loading',player:'b',reason:'game-rejoined'},
    {kind:'resume_ready',player:'a',sequence:1}, {kind:'resume_ready',player:'b',sequence:2},
    {kind:'resume_ready',player:'a',sequence:2}, {kind:'advance',now:10010},
    {kind:'advance',now:12010},
  ]),
  scenario('resume-grace-sequence-wraps-without-reusing-zero', [
    {kind:'sequence',value:2147483647}, {kind:'loading',player:'a',reason:'game-started'},
    {kind:'loading',player:'b',reason:'game-rejoined'},
  ]),
]
const path = resolve(root, 'crates/launcher/tests/fixtures/website/host-control-goldens.json')
mkdirSync(dirname(path), { recursive: true })
writeFileSync(path, `${JSON.stringify({ source:'Website/frontend/src/game/host/game-host.ts',
  sourceSha256:createHash('sha256').update(source).digest('hex'), generator:'tools/website-oracle/host-control-goldens.mjs', scenarios })}\n`)
console.log(`wrote ${scenarios.length} host control scenarios / ${scenarios.reduce((sum,row) => sum+row.steps.length,0)} checkpoints`)
