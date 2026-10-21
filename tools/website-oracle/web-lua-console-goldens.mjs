// The actual game Console VM: enabled SDK, retained globals, scheduling and budgets.
import { createRequire } from 'node:module'
import { resolve } from 'node:path'
import { loadKernel, websiteDir, writeFixture } from './oracle.mjs'
const require = createRequire(resolve(websiteDir, 'frontend/package.json'))
const { WebLuaRuntime } = await loadKernel('host/lua/web-lua-runtime.ts')
const wasmPath = require.resolve('wasmoon/dist/glue.wasm')
const player = { currentHealth: 30, currentMana: 70, deathTick: 0, discipline: 'arcane', displayName: 'Lua Wizard', element: 'fire', experience: 0, gold: 10000, id: 'player-1', level: 1, lifeState: 'alive', maximumHealth: 50, maximumMana: 100, pendingLevelUp: false, position: { x: 10, y: 20 } }
const base = { authorityPlayerId: 'player-1', boneyardSeed: null, enemies: [], multiplayer: false, phase: 'hub', playerCount: 1, players: [player], runId: null, scene: 'hub.courtyard', tick: 0, waves: null, world: 'hub' }
const boneyard = { ...base, boneyardSeed: '0000002a' + '00'.repeat(12), enemies: [{ health: 10, id: 7, lifeState: 'alive', maximumHealth: 10, position: { x: 30, y: 40 }, token: 'SKELETON' }], phase: 'active', runId: 'run-lua', scene: 'Random Boneyard', waves: { phase: 'wave-threshold', wave_ordinal: 1 }, world: 'boneyard' }
const cases = []
async function record(name, actions, { developer = false, frame = base, clockStep = 0 } = {}) {
  let currentFrame = structuredClone(frame), now = 0
  const logs = [], completions = [], bots = [], rows = []
  const runtime = await WebLuaRuntime.create({ bindings: { getAuthorityPlayerId: () => currentFrame.authorityPlayerId, getFrame: () => currentFrame },
    ...(developer ? { developer: { summonBot: config => { bots.push(config); const bot = { ...player, id: `bot-${bots.length}`, displayName: 'Test Bot' }; currentFrame = { ...currentFrame, players: [...currentFrame.players, bot], playerCount: currentFrame.playerCount + 1 }; return { display_name: bot.displayName, player_id: bot.id } } } } : {}),
    log: (level, event, detail) => logs.push({ level, event, detail }), now: () => { const value = now; now += clockStep; return value }, wasmPath })
  for (let index = 0; index < actions.length; index++) {
    const action = actions[index]
    let result = null
    if (action.kind === 'execute' || action.kind === 'enqueue') {
      result = runtime.enqueueExecution({ code: action.code, playerId: action.playerId ?? 'player-1', respond: value => completions.push({ requestId: index, result: value }) })
      if (action.kind === 'execute') { currentFrame = { ...currentFrame, tick: action.tick }; runtime.beginTick(action.tick) }
    } else if (action.kind === 'tick') { currentFrame = { ...currentFrame, tick: action.tick }; runtime.beginTick(action.tick) }
    else if (action.kind === 'frame') currentFrame = { ...currentFrame, ...action.frame }
    else if (action.kind === 'dispatch') runtime.dispatch(action.name, action.payload, action.playerId)
    else if (action.kind === 'restore') { try { runtime.restoreState(action.state) } catch (error) { result = error.message } }
    else if (action.kind === 'wants') result = runtime.wantsEvent(action.name)
    else if (action.kind === 'close') runtime.close()
    const { callbackCount, pendingExecutions, stateBytes, timerCount, budgetExceededCount } = runtime.metrics
    rows.push({ result, completions: completions.splice(0), commands: runtime.drainCommands(), logs: logs.splice(0), state: runtime.snapshotState(), bots: bots.splice(0), metrics: { callbackCount, pendingExecutions, stateBytes, timerCount, budgetExceededCount } })
  }
  runtime.close()
  cases.push({ name, developer, frame, clockStep, actions, rows })
}
const exec = (code, tick = 1) => ({ kind: 'execute', code, tick })
const nextWaveFrame = { ...boneyard, waves: { spawn_next_available: true, wave_event_id: 7, wave_ordinal: 4 } }
await record('current-next-wave-generation-and-admission', [
  exec('return sd.runtime.has_capability("waves.spawn"),sd.waves.spawn_next()'),
  exec('return sd.waves.spawn_next("ignored")', 2),
  ...[
    { world: 'hub' }, { phase: 'game-over' }, { runId: null }, { waves: null },
    { waves: { spawn_next_available: false, wave_event_id: 7 } },
    { waves: { spawn_next_available: true, wave_event_id: -1 } },
    { waves: { spawn_next_available: true, wave_event_id: 1.5 } },
    { waves: { spawn_next_available: true, wave_event_id: 9007199254740992 } },
    { waves: { spawn_next_available: true, wave_event_id: '7' } },
    { waves: { spawn_next_available: true } },
  ].flatMap((overrides, index) => [
    { kind: 'frame', frame: { ...nextWaveFrame, ...overrides } },
    exec('return sd.waves.spawn_next()', index + 3),
  ]),
], { frame: nextWaveFrame })
await record('ordinary-sandbox-and-retained-globals', [exec(`persistent=41; print('lua',_VERSION); return _VERSION, sd.runtime.api_version, sd.runtime.get_capabilities(), sd.runtime.get_mod(), type(sd.dev), type(sd.bots), type(sd.events.filter), {io=io,os=os,package=package,require=require,load=load,debug=debug,coroutine=coroutine,audio=sd.audio,draw=sd.draw}`), exec('persistent=persistent+1; return persistent', 2)])
await record('all-live-semantic-reads', [exec(`return sd.player.get_state(), sd.player.list(), sd.gameplay.get_state(), sd.gameplay.get_combat_state(), sd.hub.get_surface_state(), sd.runtime.get_frame_state(), sd.runtime.get_multiplayer_state(), sd.runtime.has_capability('world.read'), sd.runtime.has_capability('missing'), sd.scene.get_state(), sd.world.get_scene(), sd.world.get_state(), sd.world.list_actors(), sd.waves.get_state(), sd.enemies.list(), sd.enemies.get('SKELETON')`)], { frame: boneyard })
await record('source-developer-catalogs-and-real-delegation', [exec(`local items=sd.dev.list_items();local skills=sd.dev.list_skills();local welds=sd.dev.list_welds();local bot=sd.bots.summon();return #items, items[1], items[12], #skills,skills[1],skills[#skills],#welds,welds[1],welds[#welds],bot,sd.player.list()`)], { developer: true })
await record('all-queued-resource-and-developer-mutations', [exec(`return sd.player.restore_health(10),sd.player.restore_mana()`), exec(`return sd.player.set_mana(75),sd.player.set_gold(1234),sd.player.grant_experience(50),sd.dev.grant_gold(250),sd.dev.grant_item('health-potion',3),sd.dev.grant_item('equipment:0'),sd.dev.grant_skill(72,2),sd.dev.grant_weld(1000)`, 2)], { developer: true })
await record('boneyard-spawn-and-source-identities', [exec(`return sd.enemies.spawn('skeleton',{x=60,y=70}),sd.enemies.get(2001),sd.enemies.get('missing'),sd.enemies.get({}),sd.enemies.spawn('DEMON',{x=-1,y=0}).request_id`)], { frame: boneyard })
await record('state-revisions-and-partial-error-effects', [exec(`sd.state.set('counter',1);sd.state.set('counter',1); sd.state.set('nested',{list={1,true,'a'},text='x'});return sd.state.get_revision(),sd.state.get('missing','fallback'),sd.state.is_authority(),sd.state.snapshot()`), exec(`print('before'); sd.player.set_gold(10);sd.state.set('kept',true); error('after')`,2), exec(`return sd.state.delete('counter'),sd.state.delete('missing'),sd.state.clear(),sd.state.clear(),sd.state.get_revision()`,3), {kind:'restore',state:{ restored:true, value:7 }}, exec('return sd.state.snapshot(),sd.state.get_revision()',4)])
await record('timer-order-sequence-and-failing-retirement', [exec(`local order='';sd.events.on('runtime.tick',function(e) sd.state.set('event',e);sd.state.set('order',(sd.state.get('order',''))..'e') end);sd.timer.after(0,function() sd.state.set('order',sd.state.get('order','')..'a');print('after') end);sd.timer.every(11,function()sd.state.set('order',sd.state.get('order','')..'r')end);sd.timer.sequence({{delay_ms=0,callback=function()sd.state.set('step','first')end},{delay_ms=10,callback=function()sd.state.set('step','last')end}});sd.timer.after(0,function()error('timer failed')end);return sd.state.get_revision()`),{kind:'tick',tick:2},{kind:'tick',tick:3},exec('return sd.timer.clear()',4)])
await record('events-frame-boundary-and-current-authority', [exec(`sd.events.on('run.started',function(e) sd.state.set('run',e.run_id);sd.state.set('tick',sd.runtime.get_frame_state().tick) end);sd.events.on('run.started',function() error('failed callback') end); return sd.runtime.get_frame_state().tick`),{kind:'wants',name:'run.started'},{kind:'frame',frame:{tick:99}},{kind:'dispatch',name:'run.started',payload:{run_id:'run-a',absent:null}},{kind:'dispatch',name:'run.started',payload:{run_id:'run-b'}},{kind:'wants',name:'enemy.death'}])
await record('seed-selection-and-lifecycle', [exec('return sd.rng.set_seed(42),sd.rng.get_seed()'),{kind:'wants',name:'run.started'},{kind:'dispatch',name:'run.started',payload:{run_id:'run-a'}},{kind:'dispatch',name:'run.ended',payload:{}},exec('return sd.rng.get_seed()',2),{kind:'wants',name:'run.started'}])
await record('request-authority-limit-and-close', [...Array.from({length:9},()=>({kind:'enqueue',code:'sd.player.set_gold(1)'})),{kind:'frame',frame:{authorityPlayerId:'player-2'}},{kind:'tick',tick:1},{kind:'close'},exec('return 1',2)])
await record('timer-cancelled-snapshot-still-runs-once', [exec(`local b;sd.timer.after(0,function()sd.timer.cancel(b)end);b=sd.timer.after(0,function()sd.state.set('ran',true)end)`),{kind:'tick',tick:2}])
await record('callback-budget-and-registration-order', [exec(`for i=1,70 do sd.events.on('run.started',function()sd.state.set('n',sd.state.get('n',0)+1)end)end`),{kind:'dispatch',name:'run.started',payload:{}}])
await record('tick-work-budget', [exec(`sd.events.on('run.started',function()sd.state.set('n',sd.state.get('n',0)+1)end)`),{kind:'dispatch',name:'run.started',payload:{}}],{clockStep:5})
const invalid = [
  [`sd.player.restore_mana(1)`], [`sd.player.get_state('missing')`], [`sd.player.set_gold(-1)`], [`sd.player.restore_health(0/0)`], [`sd.player.set_mana(10000001)`],
  [`sd.state.set('nil',nil)`], [`sd.state.set('',1)`], [`return function()end`], [`local a={};a.a=a;return a`], [`return 1/0`], [`return string.rep('a',16385)`],
  [`local a={};for i=1,129 do a[i]=i end;return a`], [`local a={};for i=1,129 do a['k'..i]=i end;return a`], [`local a=1;for i=1,17 do a={a}end;return a`],
  [`print(string.rep('a',4097))`], [`for i=1,65 do print('x')end`], [`for i=1,5 do print(string.rep('a',4000))end`],
  [`sd.events.on('missing',function()end)`], [`sd.events.on('enemy.death',true)`], [`for i=1,129 do sd.events.on('enemy.death',function()end)end`],
  [`sd.timer.after(0,true)`], [`sd.timer.after(-1,function()end)`], [`sd.timer.after(1.5,function()end)`], [`sd.timer.every(0,function()end)`], [`sd.timer.after(86400001,function()end)`], [`sd.timer.sequence({})`], [`sd.timer.sequence({{delay_ms=0,callback=true}})`], [`sd.timer.cancel(0)`],
  [`sd.rng.set_seed(0)`], [`sd.rng.set_seed(1073741824)`], [`sd.enemies.spawn('skeleton',{x=0,y=0})`],
  [`sd.dev.grant_item('missing')`,true], [`sd.dev.grant_item('health-potion',101)`,true], [`sd.dev.grant_skill(52)`,true], [`sd.dev.grant_weld(999)`,true], [`sd.dev.grant_gold(1,'missing')`,true], [`sd.bots.summon({extra=true})`,true], [`sd.bots.summon({discipline='battle'})`,true], [`sd.bots.summon({element='void'})`,true],
  [`for i=1,257 do sd.player.set_gold(1)end`],
]
for(let i=0;i<invalid.length;i++) await record(`enabled-validation-${i}`, [exec(invalid[i][0])],{developer:invalid[i][1]??false})
writeFixture('web-lua-console-goldens.json','web-lua-console-goldens.mjs','website-rust-current/host/lua/web-lua-{runtime,api,values}.ts',
  {sourceRevision:'fa08ae249ff5563570bdb2a653e2ef9358ce7b83',cases})
