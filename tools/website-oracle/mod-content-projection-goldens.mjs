// Decode actual client content metadata through the unchanged public Source codec.
import { loadKernel, writeFixture } from './oracle.mjs'

const { decodeServerGameMessage } = await loadKernel('protocol/game-protocol.ts')
const { MOD_CONTENT_KINDS } = await loadKernel('protocol/game-mod-contract.ts')
const modId = 'parity.catalog'
const content = MOD_CONTENT_KINDS.map((contentKind, index) => ({
  art: [], contentId: String(1000000000000000000n + BigInt(index)), contentKind,
  description: `${contentKind} description`, key: contentKind, modId,
  name: `${contentKind} name`, presentation: contentKind === 'ui' ? 'prefab.minimap' : null,
}))
const id = kind => content.find(entry => entry.contentKind === kind).contentId
const paintedId = '2000000000000000000'
content.push({art: [{path: 'art/vow.png', slot: 'icon'}], contentId: paintedId,
  contentKind: 'boast', description: '', key: 'painted', modId, name: 'Painted vow', presentation: null})
const frame = {centerOffsetX: .5, centerOffsetY: -1, contentHeight: 16, contentWidth: 16,
  height: 16, logicalHeight: 24, logicalWidth: 24, width: 16, x: 3, y: 4}
const boast = {contentId: id('boast'), failureProducers: ['mana-underflow', 'potion-use'],
  icon: {kind: 'stock', record: 93, style: 3}, instruction: 'Keep this promise', modId,
  name: 'boast name', randomSkillChoices: false, response: 'Agreed', scoreMultiplier: 1.5,
  statement: 'I will take care', successWave: 12}
const complete = {boasts: [boast, {...boast, contentId: paintedId, name: 'Painted vow',
  icon: {kind: 'mod', frame, imageHeight: 32, imagePath: 'art/vow.png', imageWidth: 32}}],
  content, manifestSha256: 'AB'.repeat(32),
  powerups: [{contentId: id('powerup'), id: 12, spawnedTick: 2, x: 40.5, y: -12}], revision: 7,
  // Source permits any known content here, not only a status definition.
  statuses: [{contentId: id('item'), expiresTick: 20, instanceId: 2, startedTick: 1, targetId: 'player-1'}]}
const cases = []
function record(name, input) {
  let output = null, error = null
  try {
    const {type, ...projection} = decodeServerGameMessage(JSON.stringify({type: 'server-mod-content', ...input}))
    if (type !== 'server-mod-content') throw new Error('wrong Source message')
    output = projection
  } catch (caught) { error = caught.message }
  cases.push({name, input, output, error})
}
function change(name, mutate) {
  const input = structuredClone(complete)
  mutate(input)
  record(name, input)
}
record('empty', {boasts: [], content: [], manifestSha256: '00'.repeat(32), powerups: [], revision: 0, statuses: []})
record('all-sixteen-kinds-and-both-boast-icons', complete)
change('source-integers-above-u64', value => {
  value.revision = 1e24
  value.powerups[0].id = 1e23
  value.powerups[0].spawnedTick = 1e22
  Object.assign(value.statuses[0], {instanceId: 1e21, startedTick: 1e22, expiresTick: 1e23})
})
change('utf16-name-boundary', value => {value.content[0].name = '😃'.repeat(64)})
change('unexpected-message-field', value => {value.extra = true})
change('unknown-content-kind', value => {value.content[0].contentKind = 'new-kind'})
change('invalid-content-identity', value => {value.content[0].contentId = '0'})
change('duplicate-before-malformed-art', value => {
  value.content[1].contentId = value.content[0].contentId
  value.content[1].art = null
})
change('duplicate-art-slot', value => {value.content.at(-1).art.push({path: 'art/other.png', slot: 'icon'})})
change('invalid-art-path', value => {value.content.at(-1).art[0].path = 'other/icon.png'})
change('description-byte-boundary', value => {value.content[0].description = '猫'.repeat(342)})
change('utf16-name-overflow', value => {value.content[0].name = '😃'.repeat(65)})
change('description-before-missing-presentation', value => {
  value.content[0].description = false
  delete value.content[0].presentation
})
change('boast-content-reference', value => {value.boasts[0].contentId = id('item')})
change('duplicate-boast', value => {value.boasts.push(value.boasts[0])})
change('duplicate-failure-producer', value => {value.boasts[0].failureProducers = ['potion-use', 'potion-use']})
change('stock-icon-record-mismatch', value => {value.boasts[0].icon.record = 90})
change('custom-icon-art-mismatch', value => {value.boasts[1].icon.imagePath = 'art/other.png'})
change('custom-frame-outside-image', value => {value.boasts[1].icon.frame.x = 30})
change('custom-frame-negative-position', value => {value.boasts[1].icon.frame.y = -1})
change('unknown-frame-field', value => {value.boasts[1].icon.frame.extra = 1})
change('boast-score-range', value => {value.boasts[0].scoreMultiplier = .5})
change('powerup-content-family', value => {value.powerups[0].contentId = id('item')})
change('duplicate-powerup-id', value => {value.powerups.push(value.powerups[0])})
change('status-expiry-order', value => {value.statuses[0].expiresTick = 1})
change('reserved-status-player', value => {value.statuses[0].targetId = 'constructor'})
change('status-array-bound', value => {value.statuses = Array(4097).fill(null)})
change('negative-revision', value => {value.revision = -1})

writeFixture('mod-content-projection-goldens.json', 'mod-content-projection-goldens.mjs',
  'protocol/game-protocol.ts + codecs/mod-content.ts + codecs/items.ts + codecs/values.ts', {cases})
