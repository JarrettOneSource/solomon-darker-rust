import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { fixturesDir, loadKernel, writeFixture } from './oracle.mjs'

const chat = await loadKernel('hub-npc-dialogue.ts')
const { createNativeHubNpcState } = await loadKernel('core-kernels/native-hub-npc.ts')
const { modContentProjection } = await loadKernel('protocol/codecs/mod-content.ts')
const sourceFixture = JSON.parse(readFileSync(resolve(fixturesDir, 'mod-content-projection-goldens.json')))
const mods = modContentProjection(sourceFixture.cases.find(row => row.name === 'all-sixteen-kinds-and-both-boast-icons').input)
const selectors = []
for (const metadata of [null, mods]) {
  for (const selector of ['boast', 'books', 'teacher-spells']) {
    for (const read of [false, true]) {
      for (const advancedUnlocks of [Array(8).fill(false), Array(8).fill(true)]) {
        const npc = { ...createNativeHubNpcState(), librarianLaceRead: read }
        const rows = chat.hubNpcSelectorRows(selector, npc, { advancedUnlocks }, metadata)
        selectors.push({ metadata, selector, read, advancedUnlocks, rows,
          responses: rows.map(row => ({ id: row.id, response: chat.hubNpcSelectorResponse(selector, row.id, metadata), action: chat.hubNpcSelectorAction(selector, row.id) })),
        })
      }
    }
  }
}
const selections = []
const ids = [null, 0, 1, 2, 3, 4, 99, ...mods.boasts.map(row => ({ kind: 'mod', modId: row.modId, contentId: row.contentId })), { kind: 'mod', modId: 'wrong.owner', contentId: mods.boasts[0].contentId }]
for (const metadata of [null, mods]) {
  for (const selected of ids) {
    for (const failed of [false, true]) {
      const state = { ...createNativeHubNpcState().boast, failed, selected }
      selections.push({ metadata, selected, failed, instruction: chat.hubBoastInstruction(selected, metadata), failure: chat.hubBoastFailureText(state, metadata) })
    }
  }
}
writeFixture('mod-boast-ui-goldens.json', 'tools/website-oracle/mod-boast-ui-goldens.mjs', 'Website hub-npc-dialogue.ts using public ModContent codec and native-hub-npc state', { selectors, selections })
console.log(`${selectors.length} selectors, ${selections.length} instruction/failure frames`)
