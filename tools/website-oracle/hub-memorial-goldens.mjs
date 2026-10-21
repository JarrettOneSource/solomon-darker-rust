import { loadKernel, writeFixture } from './oracle.mjs'
import { memorialPortrait as portrait, memorialSnapshot as project } from './hub-memorial-rows.mjs'
const memorial = await loadKernel('core-kernels/hub-memorial.ts')

const makePortrait = index => ({
  accountUsername: index % 2 ? null : 'memorial-account', awesomeness: index * 17,
  awesomestKill: index % 2 ? null : 'Demon', capturedAtTick: 900 + index,
  config: { displayName: `Wizard ${index}`, discipline: 'arcane', element: 'fire' },
  elapsedTicks: 500 + index, equipment: {
    hat: index % 2 ? null : { primaryTint: 0x123456, secondaryTint: 0xffffff, selector: 2 },
    robe: { primaryTint: 0xabcdef, secondaryTint: 0x654321, selector: 1 },
    weapon: index % 3 ? { kind: 'wand', selector: 5 } : null,
  },
  headingIndex: index % 24, level: 3, monstersKilled: index * 2,
  playerId: `player-${index % 2}`, portraitScale: 0.75, runId: `run-${index}`, wave: 5,
})

let state = memorial.createHubMemorialState()
const initial = project(state)
const steps = []
for (let index = 0; index < 13; index += 1) {
  const input = makePortrait(index)
  const markerDraw = index % 5
  const next = memorial.archiveHubMemorialPortrait(state, input, markerDraw)
  steps.push({ input: portrait(input), markerDraw, changed: next !== state, state: project(next) })
  state = next
  if (index === 3) {
    // A later capture of the same run/player must not replace another slot.
    const duplicate = { ...input, capturedAtTick: 99999, awesomeness: 99999 }
    const nextDuplicate = memorial.archiveHubMemorialPortrait(state, duplicate, 4)
    steps.push({ input: portrait(duplicate), markerDraw: 4, changed: nextDuplicate !== state, state: project(nextDuplicate) })
    state = nextDuplicate
  }
}
let ties = memorial.createHubMemorialState()
ties = { ...ties, slots: ties.slots.map(slot => ({ ...slot, age: 0 })) }
const tieInput = makePortrait(20)
const tieResult = memorial.archiveHubMemorialPortrait(ties, tieInput, 3)

writeFixture('hub-memorial-goldens.json', 'hub-memorial-goldens.mjs', 'core-kernels/hub-memorial.ts', {
  initial, steps, tie: { before: project(ties), input: portrait(tieInput), markerDraw: 3, state: project(tieResult) },
  interactions: [...memorial.HUB_MEMORIAL_INTERACTION_IDS, 'painting-2', 'painting-109', 'missing'].map(id => ({ id, slot: memorial.hubMemorialSlotIndexForInteraction(id) })),
})
