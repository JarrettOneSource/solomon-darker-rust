// Record the unchanged private eligibility/issuance functions. The boundary supplies
// their small score facts and protocol player projection; no game tick is simulated here.
import { createHash } from 'node:crypto'
import { readFileSync } from 'node:fs'
import { createRequire } from 'node:module'
import { resolve } from 'node:path'
import { gameDir, loadKernel, websiteDir, writeFixture } from './oracle.mjs'

const require = createRequire(resolve(websiteDir, 'frontend/package.json'))
const ts = require('typescript')
const source = readFileSync(resolve(gameDir, 'host/game-host.ts'), 'utf8')
const tree = ts.createSourceFile('game-host.ts', source, ts.ScriptTarget.Latest, true)
const names = new Set([
  'taintActiveRun', 'taintIneligibleClientRuns', 'publishLeaderboardReceipts',
  'pruneLeaderboardRunState',
])
const functions = []
function visit(node) {
  if (ts.isFunctionDeclaration(node) && node.name && names.has(node.name.text)) {
    functions.push(node.getText(tree))
    names.delete(node.name.text)
  }
  ts.forEachChild(node, visit)
}
visit(tree)
if (names.size) throw new Error(`Missing Source functions: ${[...names]}`)
const { completedHallOfFameEntry } = await loadKernel('hall-of-fame-entry.ts')
const { createGameLeaderboardReceipt } = await loadKernel('host/game-leaderboard-receipt.ts')

const secret = 'source-leaderboard-policy-32-byte-secret'
const program = `
const { completedHallOfFameEntry, createGameLeaderboardReceipt } = bindings;
return function create() {
  const options = { leaderboardReceiptSecret: ${JSON.stringify(secret)} };
  const clients = new Map();
  const leaderboardIneligibleRunIds = new Set();
  const issuedLeaderboardReceipts = new Set();
  let state = {world:{kind:'hub'}};
  let sharedWorlds = null;
  let recipients = [];
  let clockReads = 0;
  const Date = class { toISOString() { clockReads++; return '2026-10-21T12:34:56.007Z'; } };
  const WebSocket = {OPEN:1};
  const encodeGameMessage = message => message;
  const authorityForPlayer = () => null;
  const createGameSnapshot = completed => completed.projection;
  const stateForPlayer = id => {
    const runId = clients.get(id).runId;
    return {world:runId === null ? {kind:'hub'} : {kind:'boneyard',runId}};
  };
  ${functions.join('\n')}
  function scoreState(frame) {
    if (frame === null) return {world:{kind:'hub'}};
    const hallOfFameRuns = {};
    const players = {};
    for (const [id, elapsedTicks] of Object.entries(frame.scores)) {
      hallOfFameRuns[id] = {
        elapsedTicks, portraitHeadingIndex: frame.portrait === false ? null : 7,
        portraitScale: frame.portrait === false ? null : 1, awesomeness:500,
        awesomestKill:'Skeleton', monstersKilled:12,
      };
      players[id] = {config:{displayName:id,element:'fire',discipline:'arcane'},
        progression:{level:3,learnedSkillOrder:[4,1],learnedSkills:[[4,2],[1,1]]},
        economy:{ownedPerkSelectors:[1,7]}};
    }
    const world = {kind:'boneyard',runId:frame.runId,hallOfFameRuns,waves:{waveOrdinal:7}};
    return {world,projection:{world,players,run:{phase:frame.phase ?? 'game-over'}}};
  }
  return {step(action) {
    recipients = []; clockReads = 0;
    switch (action.kind) {
      case 'clients':
        clients.clear();
        for (const row of action.clients) clients.set(row.id, {
          playerId:row.id, globalScoreEligible:row.eligible, partyRejoinSlot:row.staged ? {} : null,
          leaderboardUserId:row.userId,onlinePreferences:{submitRuns:row.submit},runId:row.runId,
          socket:{readyState:row.open ? 1 : 3,send() { recipients.push(row.id); }},
        });
        break;
      case 'taint': taintIneligibleClientRuns(); break;
      case 'tutorial': leaderboardIneligibleRunIds.add(action.runId); break;
      case 'publish':
        options.leaderboardReceiptSecret = action.enabled === false ? undefined : ${JSON.stringify(secret)};
        publishLeaderboardReceipts(scoreState(action.previous),scoreState(action.completed));
        break;
      case 'prune':
        sharedWorlds = action.shared ? {runs:action.active.map(runId => ({state:{world:{kind:'boneyard',runId}}}))} : null;
        state = {world:action.active.length ? {kind:'boneyard',runId:action.active[0]} : {kind:'hub'}};
        pruneLeaderboardRunState();
        break;
      default: throw new Error('Unrecorded action '+action.kind);
    }
    return {recipients,clockReads,ineligible:[...leaderboardIneligibleRunIds].sort(),
      issued:[...issuedLeaderboardReceipts].sort()};
  }};
};`
const compiled = ts.transpileModule(program, {compilerOptions:{target:ts.ScriptTarget.ES2022}}).outputText
const create = new Function('bindings', compiled)({completedHallOfFameEntry,createGameLeaderboardReceipt})
const client = (id, patch = {}) => ({id,userId:42,eligible:true,staged:false,runId:'run-a',submit:true,open:true,...patch})
const run = (scores, patch = {}) => ({runId:'run-a',scores,...patch})
const publish = (previous, completed, patch = {}) => ({kind:'publish',previous,completed,...patch})
function scenario(name, actions) {
  const owner = create()
  return {name,steps:actions.map(action => ({action,expected:owner.step(action)}))}
}
const scenarios = [
  scenario('one-receipt-per-completed-edge-in-client-creation-order', [
    {kind:'clients',clients:[client('b',{userId:43}),client('a')]},
    publish(run({a:null,b:null}),run({a:null,b:null})),
    publish(run({a:null,b:null}),run({a:900,b:900})),
    publish(run({a:null,b:null}),run({a:900,b:900})),
    publish(run({a:900,b:900}),run({a:900,b:900})),
    {kind:'prune',shared:true,active:['run-a']},
    {kind:'prune',shared:true,active:[]},
    publish(run({a:null}),run({a:901})),
  ]),
  scenario('account-submit-socket-and-missing-score-are-independent', [
    {kind:'clients',clients:[client('a',{userId:null}),client('b',{submit:false}),
      client('c',{open:false}),client('d'),client('e')]},
    publish(run({a:null,b:null,c:null,d:null}),run({a:900,b:900,c:900,e:900})),
    {kind:'clients',clients:[client('a'),client('b'),client('c'),client('d'),client('e')]},
    publish(run({a:900,b:900,c:900,e:900}),run({a:900,b:900,c:900,d:900,e:900})),
  ]),
  scenario('one-ineligible-member-taints-the-whole-retained-run', [
    {kind:'clients',clients:[client('a'),client('b',{eligible:false})]}, {kind:'taint'},
    {kind:'clients',clients:[client('a')]},
    publish(run({a:null}),run({a:900})),
    {kind:'prune',shared:true,active:['run-a','run-b']},
    {kind:'prune',shared:true,active:['run-b']},
    publish(run({a:null}),run({a:900})),
  ]),
  scenario('hub-and-staged-rejoin-never-taint-an-unmaterialized-run', [
    {kind:'clients',clients:[client('a',{eligible:false,runId:null}),
      client('b',{eligible:false,staged:true})]}, {kind:'taint'},
    {kind:'clients',clients:[client('a',{eligible:false})]}, {kind:'taint'},
    {kind:'prune',shared:false,active:[]},
  ]),
  scenario('tutorial-taint-is-explicit-and-private-pruning-retains-only-current-run', [
    {kind:'tutorial',runId:'run-a'}, {kind:'tutorial',runId:'run-b'},
    {kind:'clients',clients:[client('a')]},
    {kind:'prune',shared:false,active:['run-a']},
    publish(run({a:null}),run({a:900})),
    {kind:'prune',shared:false,active:[]},
  ]),
  scenario('same-world-secret-and-archived-portrait-are-required', [
    {kind:'clients',clients:[client('a')]},
    publish(null,run({a:900})), publish(run({a:null}),null),
    publish(run({a:null}),run({a:900},{runId:'run-b'})),
    publish(run({a:null}),run({a:900}),{enabled:false}),
    publish(run({a:null}),run({a:900},{phase:'active'})),
    publish(run({a:null}),run({a:900},{portrait:false})),
    publish(run({a:null}),run({a:900})),
  ]),
]
writeFixture('host-leaderboard-goldens.json','tools/website-oracle/host-leaderboard-goldens.mjs',
  'Website/frontend/src/game/host/game-host.ts; hall-of-fame-entry.ts; host/game-leaderboard-receipt.ts',
  {sourceSha256:createHash('sha256').update(source).digest('hex'),scenarios,
    utc:[0,1,999,1000,-1,Date.UTC(2026,9,21,12,34,56,7)]
      .map(milliseconds => ({milliseconds,expected:new Date(milliseconds).toISOString()}))})
console.log(`${scenarios.length} source score policies / ${scenarios.reduce((n,row)=>n+row.steps.length,0)} observations`)
