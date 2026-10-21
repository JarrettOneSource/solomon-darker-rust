import { loadKernel, writeFixture } from './oracle.mjs'
const manager = await loadKernel('core-kernels/native-world-manager-order.ts')
const initial = { actor: 49, transient: 2 }
const order = manager.createNativeWorldManagerOrder({ nextRegistrationOrdinal: initial })
const batches = [['actor', 2], ['transient', 3], ['actor', 0], ['actor', 1], ['transient', 1]]
const samples = []
for (const [lane, count] of batches) {
  const registrations = order.registerMany(lane, count)
  samples.push({ lane, count, ordinals: registrations.map(item => item.registrationOrdinal),
    next: order.state().nextRegistrationOrdinal })
}
writeFixture('world-painter-goldens.json', 'tools/website-oracle/world-painter-goldens.mjs',
  'Website/frontend/src/game/core-kernels/native-world-manager-order.ts', { initial, samples })
