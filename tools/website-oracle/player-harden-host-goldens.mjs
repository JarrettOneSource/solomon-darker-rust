// Full source coating synchronization, shared effect IDs, registrations and event order.
import { loadKernel, writeFixture } from './oracle.mjs'
import { setup } from './player-entity-fixture.mjs'
const entities = await loadKernel('core-server/player-entity-store.ts')
const effects = await loadKernel('core-kernels/native-harden-effects.ts')
const host = await loadKernel('core-server/player-harden-effects.ts')
const secondary = await loadKernel('core-kernels/native-secondary-abilities.ts')
const painter = await loadKernel('core-kernels/native-world-manager-order.ts')
const nativeRng = await loadKernel('core-kernels/native-rng.ts')
const cases = []
const position = { x: 321.25, y: 245.5 }
for (const before of [null, 0, 0.005, 0.249, 0.25, 0.5, 1]) {
  for (const after of [0, 0.005, 0.25, 1]) for (const withChip of [false, true]) {
    let rng = nativeRng.createNativeRng(917)
    const chips = []
    if (withChip) {
      let chip
      do {
        const created = effects.createNativeHardenChip(position, rng)
        rng = created.rng
        chip = created.chip
      } while (chip === null)
      chips.push({ chip, ownerId: 'local', position, worldKey: 'boneyard:oracle' })
    }
    const beforeRng = rng
    const order = painter.createNativeWorldManagerOrder({ nextRegistrationOrdinal: { actor: 300, transient: 500 } })
    const result = host.synchronizePlayerHardenEffects({
      before: before === null ? entities.createPlayerEntityStore() : setup({ runtime: { harden: { armor: 2, coating: before } } }),
      after: setup({ runtime: { harden: { armor: 2, coating: after } } }), chips,
      spells: { nextId: 81, projectiles: [], transients: [] },
      secondary: secondary.createNativeSecondarySimulation(55), tick: 202, rng,
      register: order.register, worldKey: () => 'boneyard:oracle',
    })
    cases.push({ before, after, chips, beforeRng, expected: {
      spells: result.spells, secondary: result.secondary, rng: result.rng, order: order.state(),
    } })
  }
}
writeFixture('player-harden-host-goldens.json', 'tools/website-oracle/player-harden-host-goldens.mjs',
  'Website core-server/player-harden-effects.ts complete synchronization (pinned ed0a2d59)', { cases })
