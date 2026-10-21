// Actual host snapshots consume owned stat books for secondary supplements.
import { loadKernel, writeFixture } from './oracle.mjs'
const { createGameSimulation } = await loadKernel('core-server/game-simulation.ts')
const { createGameSnapshot } = await loadKernel('host/game-snapshot.ts')
const { NATIVE_SECONDARY_ABILITY_IDS } = await loadKernel('core-kernels/native-secondary-ability-contract.ts')
const cases=[]
for(const authored of [false,true]) for(const upgradeRank of [0,1,50]) {
 const state=createGameSimulation()
 const book=state.playerEntities.skillBooks[0], stats=structuredClone(state.playerEntities.statBooks[0])
 const ranks=[...book.effectiveRanks]
 for(const id of NATIVE_SECONDARY_ABILITY_IDS) ranks[id]=1
 for(const id of [28,39,55,75]) ranks[id]=upgradeRank
 state.playerEntities.skillBooks[0]={...book,effectiveRanks:ranks,permanentRanks:ranks}
 if(authored) {
  stats.entries[28].numericProperties.mManaCost=[13,23,37]
  stats.entries[55].numericProperties.mManaCost=17.25
  stats.entries[75].numericProperties.mManaCost=[19,29,41]
 }
 state.playerEntities.statBooks[0]=stats
 const snapshot=createGameSnapshot(state,'local-player')
 cases.push({authored,upgradeRank,ranks,statBook:stats,costs:snapshot.players['local-player'].progression.secondaryManaCosts})
}
writeFixture('secondary-stat-book-goldens.json','secondary-stat-book-goldens.mjs','host/game-snapshot.ts::protocolPlayerState',{cases})
