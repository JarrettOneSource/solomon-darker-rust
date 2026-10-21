import { loadKernel, writeFixture } from './oracle.mjs'
const { NATIVE_GENERATED_BONEYARDS } = await loadKernel('host/native-generated-boneyards.ts')
const { STOCK_TUTORIAL_BONEYARD } = await loadKernel('host/stock-tutorial-boneyard.ts')
const { createBoneyardWorld } = await loadKernel('core-server/boneyard-world-construction.ts')
const seed = '00000000000000000000000000000000'
const cases = [
  { id:'default-random', source:'default', tutorial:false },
  { id:'another-default-id', source:'default', tutorial:false },
  { id:'mod:example:yard:digest', source:'mod', tutorial:false },
  { id:'stock-tutorial', source:'default', tutorial:true },
].map(input => {
  const template = input.tutorial ? STOCK_TUTORIAL_BONEYARD : NATIVE_GENERATED_BONEYARDS[0]
  const choice = { id:input.id, name:'Source ownership', source:input.source,
    ...(input.source === 'mod' ? {modId:'example',modName:'Example'} : {}) }
  const world = createBoneyardWorld({ choice, seed, runId:seed, ...template })
  return { ...input, encounter:world.encounter!==null, arena:world.arenaTransition!==null,
    waves:world.waves!==null, lantern:world.lanternPosition!==null }
})
writeFixture('boneyard-source-goldens.json','boneyard-source-goldens.mjs','core-server/boneyard-world-construction.ts',{cases})
