import {loadKernel,writeFixture} from './oracle.mjs'
const contract=await loadKernel('renderer/boneyard-render-contract.ts')
const hud=await loadKernel('native-spectator-hud.ts')
const player=(name,life,x)=>({config:{displayName:name},position:{x,y:130+x/10},progression:{lifeState:life}})
const base={players:{local:player('Local','dying',100),zeta:player('Zeta','alive',620),alpha:player('Alpha','alive',420),
  outsider:player('Outside','alive',900)},run:{phase:'active',runId:'run-one',eligiblePlayerIds:['zeta','local','alpha']},
  world:{kind:'boneyard',enemies:[{},{},{}],maggots:[{}],waves:{pendingSpawnBudget:4,waveOrdinal:7,phase:'spawning'}}}
let current=contract.INITIAL_BONEYARD_SPECTATOR_CAMERA_STATE
const cases=[]
function record(name,snapshot,advance=false,seed=current){
  current=contract.boneyardSpectatorCameraState(snapshot,'local',seed,advance)
  cases.push({name,snapshot,advance,current:seed,expected:current,
    focus:contract.boneyardCameraFocus(snapshot,'local',current,{x:33,y:44}),
    status:contract.boneyardSpectatorStatus(snapshot,'local',current)})
}
record('dying keeps its own camera',structuredClone(base))
const active=structuredClone(base);active.players.local.progression.lifeState='spectating'
record('spectating admits first semantic ID',active)
record('left or right cast start advances to next living peer',active,true)
const lethal=structuredClone(active);lethal.players.zeta.progression.lifeState='lethal-pending';record('retains a lethal target',lethal)
const dying=structuredClone(lethal);dying.players.zeta.progression.lifeState='dying';record('retains death presentation',dying)
record('explicit cycle omits a dying peer',dying,true)
record('cycle wraps',active,true)
const removed=structuredClone(active);delete removed.players.zeta;record('retarget removed peer',removed)
const waiting=structuredClone(active);waiting.players.alpha.progression.lifeState='spectating';waiting.players.zeta.progression.lifeState='spectating';record('waits without eligible living peer',waiting)
const gameOver=structuredClone(active);gameOver.run.phase='game-over';record('game over clears follow and status',gameOver)
const nextRun=structuredClone(active);nextRun.run.runId='run-two';record('new run selects anew',nextRun)
const alive=structuredClone(nextRun);alive.players.local.progression.lifeState='alive';record('respawn returns camera to local',alive)
const nullRun=structuredClone(active);nullRun.run.runId=null;record('missing run clears state',nullRun)
const noLocal=structuredClone(active);delete noLocal.players.local;record('missing local uses scene spawn',noLocal)
const opening=structuredClone(active);opening.world.enemies=[];opening.world.maggots=[{}];opening.world.waves={phase:'opening',waveOrdinal:0,pendingSpawnBudget:1};record('opening wave singular labels',opening)
const noWaves=structuredClone(active);noWaves.world.waves=null;noWaves.world.enemies=[];noWaves.world.maggots=[];record('absent director waiting text',noWaves)
const layouts=[[1600,900],[1920,1080],[1600,3466.6666666666665]].map(([width,height])=>({viewport:{width,height},expected:hud.nativeSpectatorHudLayout({width,height})}))
writeFixture('spectator-goldens.json','tools/website-oracle/spectator-goldens.mjs',
  'Website@fa08ae249ff5563570bdb2a653e2ef9358ce7b83/renderer/boneyard-render-contract.ts and native-spectator-hud.ts',{cases,layouts})
