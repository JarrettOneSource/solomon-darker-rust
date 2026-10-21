import {loadKernel,writeFixture} from './oracle.mjs'
const {gameRunWorldTick,createGameRunLifecycle}=await loadKernel('core-kernels/game-run.ts')
const rows=[]
for(const phase of ['hub','active','game-over','loadout'])
  for(const tick of [0,0.75,3.99,100.25,1000,1000.99])
    for(const gameOverTicks of [0,5,99]) {
      const run={...createGameRunLifecycle(),phase,gameOverTicks}
      rows.push({phase,tick,gameOverTicks,value:gameRunWorldTick(tick,run)})
    }
writeFixture('game-run-world-clock-goldens.json','game-run-world-clock-goldens.mjs',
  'current core-kernels/game-run.ts gameRunWorldTick',{rows})
