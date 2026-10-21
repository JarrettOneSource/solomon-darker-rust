// Original profile fields, all disk Boasts and byte-preserving native patches.
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { loadKernel, websiteDir, writeFixture } from './oracle.mjs'

const bridge = await loadKernel('save/native-save-bridge.ts')
const codec = await loadKernel('save/native-save-codec.ts')
const progression = await loadKernel('core-kernels/player-progression.ts')
const template = JSON.parse(readFileSync(resolve(websiteDir, 'frontend/public/game/native/portable-profile-template.json'), 'utf8'))
const bytes = value => new Uint8Array(Buffer.from(value, 'base64'))
const base64 = value => Buffer.from(value).toString('base64')
const darkdata = bytes(template.files.darkdata.base64)
const gamestate = bytes(template.files.gamestate.base64)
const profile = value => {
  const { buffer, ...state } = bridge.decodeNativeDarkdataProfile(value)
  return state
}
const wizard = bridge.decodeNativeGamestateWizard(gamestate)
const wizardPatch = { ...wizard, permanentRanks: wizard.rows.map(row => row.permanentRank) }
delete wizardPatch.rows
delete wizardPatch.experienceEnabled
delete wizardPatch.randomBoastActive
const profilePatch = { ...profile(darkdata) }
for (const key of ['memorialMarker', 'storageChildCount', 'storagePayloadLength']) delete profilePatch[key]
const profiles = []
for (const [name, patch] of [
  ['same disk profile', profilePatch],
  ['gold, ordered Tonic bundle and flags', { ...profilePatch, gold: 2147483647, dowsingFee: 123456,
    hagathaBundleSelectors: [21, 27, 0, 27, 1, 26],
    firstMixed: Array.from({ length: 30 }, (_, i) => i % 2 === 0),
    helpPending: Array.from({ length: 10 }, (_, i) => i % 2 === 1), tutorialPending: false, librarianLaceRead: true }],
]) {
  const output = bridge.patchNativeDarkdata(darkdata, patch)
  profiles.push({ name, patch, output: base64(output), decoded: profile(output) })
}
const concentrations = Array.from({ length: 72 }, (_, i) => i + 8).filter(id => progression.nativeSkillCategory(id) === 3)
const complexPatch = { ...wizardPatch, name: 'Mèrlin 🧙',
  level: 75, offerSeed: 999999, pendingSkillChoices: 6, deferredSkillChoices: 7,
  experience: 1234.125, previousThreshold: 1000.25, nextThreshold: 2000.5,
  currentHealth: 12.5, maximumHealth: 123.75, currentMana: 27.125, maximumMana: 222.5,
  manaCostReduction: -3.75, offensiveDamageFlat: 37.125, experienceBonus: -0.25,
  meditationIdleDelay: -1, firewalkerActive: true, weldEffect: 0.125,
  cheatDeathEnabled: true, cheatDeathCharges: 2, poisonImmunityTicks: 900,
  permanentRanks: Array.from({ length: 83 }, (_, i) => (i % 5) + 1), learnedOrder: [20, 8, 79],
  concentrationSkillIds: concentrations.slice(0, 2), nextConcentrationSlot: 1,
  skillQuickbar: [8, 9, 10, null, 13, null, 14, 79], perkSelectors: [21, 27, 27, 1, 2], perkCapacity: 9,
  hagathaOwnership: Array.from({ length: 50 }, (_, i) => [1, 2, 21, 27, 49].includes(i)),
}
const wizards = []
for (const [name, patch] of [['same wizard', wizardPatch], ['all mapped scalars and live selections', complexPatch]]) {
  const output = bridge.patchNativeGamestate(gamestate, patch)
  wizards.push({ name, patch, boast: null, output: base64(output), decoded: bridge.decodeNativeGamestateWizard(output),
    decodedBoast: bridge.decodeNativeGamestateBoast(output) })
}
for (const selected of [null, 0, 1, 2, 3, 4]) {
  for (const state of [[false, false], [false, true], [true, false]]) {
    const [failed, succeeded] = state
    if ((selected === null && (failed || succeeded)) || (selected === 3 && failed)) continue
    const boast = { failed, selected, succeeded }
    const output = bridge.patchNativeGamestate(gamestate, wizardPatch, boast)
    assert.deepEqual(bridge.decodeNativeGamestateBoast(output), boast)
    wizards.push({ name: `Boast ${selected} ${failed ? 'failed' : succeeded ? 'succeeded' : 'active'}`,
      patch: wizardPatch, boast, output: base64(output), decoded: bridge.decodeNativeGamestateWizard(output), decodedBoast: boast })
  }
}
// Larger live binding tables keep their appended opaque words and tail members.
const live = structuredClone(codec.parseNativeSyncBuffer(gamestate))
const bindings = live.root.children[1].children[0]
const payload = bindings.payload
const view = new DataView(payload.buffer, payload.byteOffset, payload.byteLength)
const boolCount = view.getUint32(0, true)
const countOffset = 4 + boolCount
const count = view.getUint32(countOffset, true)
const tail = countOffset + 4 + count * 4
const extra = Uint8Array.from([5, 0, 0, 0, 255, 255, 255, 255, 78, 86, 52, 18])
const expanded = new Uint8Array(payload.length + extra.length)
expanded.set(payload.subarray(0, tail)); expanded.set(extra, tail); expanded.set(payload.subarray(tail), tail + extra.length)
new DataView(expanded.buffer).setUint32(countOffset, count + 3, true)
live.root.children[1].children[0] = { ...bindings, payload: expanded }
const liveBytes = codec.encodeNativeSyncBuffer(live)
const livePatched = bridge.patchNativeGamestate(liveBytes, complexPatch)
wizards.push({ name: 'active binding count with opaque appended words', input: base64(liveBytes), patch: complexPatch,
  boast: null, output: base64(livePatched), decoded: bridge.decodeNativeGamestateWizard(livePatched),
  decodedBoast: bridge.decodeNativeGamestateBoast(livePatched) })

const errors = []
function reject(name, operation, input, patch = null, boast = null) {
  let message
  try {
    if (operation === 'profile') bridge.decodeNativeDarkdataProfile(input)
    if (operation === 'wizard') bridge.decodeNativeGamestateWizard(input)
    if (operation === 'boast') bridge.decodeNativeGamestateBoast(input)
    if (operation === 'patchProfile') bridge.patchNativeDarkdata(input, patch)
    if (operation === 'patchWizard') bridge.patchNativeGamestate(input, patch, boast ?? undefined)
  } catch (error) { message = error.message }
  assert.equal(typeof message, 'string', name)
  errors.push({ name, operation, input: base64(input), patch, boast, error: message })
}
function mutate(kind, edit) {
  const buffer = structuredClone(kind === 'profile' ? codec.decodeNativeDarkdata(darkdata) : codec.parseNativeSyncBuffer(gamestate))
  edit(buffer)
  return kind === 'profile' ? codec.encodeNativeDarkdata(buffer) : codec.encodeNativeSyncBuffer(buffer)
}
reject('darkdata boolean byte', 'profile', mutate('profile', buffer => { buffer.root.children[0].payload[14] = 2 }))
reject('darkdata core size', 'profile', mutate('profile', buffer => {
  buffer.root.children[0] = { ...buffer.root.children[0], payload: buffer.root.children[0].payload.slice(1) }
}))
reject('darkdata bundle repeated ordinary', 'patchProfile', darkdata, { ...profilePatch, hagathaBundleSelectors: [1, 1] })
reject('darkdata short help state', 'patchProfile', darkdata, { ...profilePatch, helpPending: [true] })
reject('darkdata short mix state', 'patchProfile', darkdata, { ...profilePatch, firstMixed: [] })
reject('gamestate root membership', 'wizard', mutate('wizard', buffer => {
  buffer.root = { ...buffer.root, children: buffer.root.children.slice(0, 7) }
}))
reject('gamestate row count', 'wizard', mutate('wizard', buffer => {
  buffer.root.children[0].children[0].payload[0] = 82
}))
reject('gamestate wizard extension', 'wizard', mutate('wizard', buffer => {
  const node = buffer.root.children[0].children[1]
  buffer.root.children[0].children[1] = { ...node, payload: node.payload.slice(1) }
}))
reject('gamestate name terminator', 'wizard', mutate('wizard', buffer => {
  const node = buffer.root.children[0]; const length = new DataView(node.payload.buffer, node.payload.byteOffset).getUint32(8, true)
  node.payload[12 + length - 1] = 1
}))
reject('gamestate replacement cursor', 'wizard', mutate('wizard', buffer => {
  buffer.root.children[7].payload[1] = 2
}))
reject('gamestate header binding disagreement', 'wizard', mutate('wizard', buffer => {
  const node = buffer.root.children[0]
  new DataView(node.payload.buffer, node.payload.byteOffset).setInt32(node.payload.length - 4, 9, true)
}))
reject('patch missing permanent row', 'patchWizard', gamestate, { ...wizardPatch, permanentRanks: [1] })
reject('patch permanent rank out of range', 'patchWizard', gamestate, { ...wizardPatch, permanentRanks: wizardPatch.permanentRanks.map((value, i) => i === 82 ? 65536 : value) })
reject('patch learned order outside disk skills', 'patchWizard', gamestate, { ...wizardPatch, learnedOrder: [80] })
reject('patch invalid Hagatha capacity', 'patchWizard', gamestate, { ...wizardPatch, perkCapacity: 4 })
reject('patch short Hagatha ownership', 'patchWizard', gamestate, { ...wizardPatch, hagathaOwnership: [] })
reject('patch empty name', 'patchWizard', gamestate, { ...wizardPatch, name: '' })
reject('patch UTF8 name ceiling', 'patchWizard', gamestate, { ...wizardPatch, name: '🧙'.repeat(64) })
reject('patch Plane Orb primary rejected', 'patchWizard', gamestate, { ...wizardPatch, selectedPrimarySkillId: 80 })
reject('patch short quickbar', 'patchWizard', gamestate, { ...wizardPatch, skillQuickbar: [8] })
reject('patch next concentration cursor', 'patchWizard', gamestate, { ...wizardPatch, nextConcentrationSlot: 2 })
reject('patch null Boast cannot fail', 'patchWizard', gamestate, wizardPatch, { selected: null, failed: true, succeeded: false })
reject('patch random Boast cannot fail', 'patchWizard', gamestate, wizardPatch, { selected: 3, failed: true, succeeded: false })
reject('patch Boast cannot fail and succeed', 'patchWizard', gamestate, wizardPatch, { selected: 4, failed: true, succeeded: true })

writeFixture('native-save-bridge-goldens.json', 'tools/website-oracle/native-save-bridge-goldens.mjs',
  'Website/frontend/src/game/save/native-save-bridge.ts', {
    base: { darkdata: base64(darkdata), gamestate: base64(gamestate), profile: profile(darkdata), wizard,
      boast: bridge.decodeNativeGamestateBoast(gamestate) }, profiles, wizards, errors,
  })
