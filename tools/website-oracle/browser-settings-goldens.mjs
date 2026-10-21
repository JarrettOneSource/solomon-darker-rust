import { loadKernel, writeFixture } from './oracle.mjs'

const settings = await loadKernel('game-settings.ts')
const fresh = () => structuredClone(settings.DEFAULT_GAME_SETTINGS)
const reads = []
function read(name, value, raw = false) {
  const serialized = raw ? value : JSON.stringify(value)
  const storage = { getItem: () => serialized, setItem() {} }
  reads.push({ name, serialized, expected: settings.readGameSettings(storage) })
}
read('default', fresh())
for (const serialized of ['', '{', 'null', 'true', '1', '[]', '"text"']) read('invalid-json-or-record-' + serialized, serialized, true)
for (const enableCheats of [true, false, null, 0, 'yes', {}]) read('cheats-only-' + JSON.stringify(enableCheats), { enableCheats })

const changed = fresh()
for (const key of Object.keys(changed)) if (typeof changed[key] === 'boolean') changed[key] = !changed[key]
Object.assign(changed, { cameraFovPercent: 125, uiScalePercent: 150, lightQualityPercent: 24, musicVolumePercent: 37, soundVolumePercent: 61 })
changed.controls = { ...changed.controls, moveUp: 'ArrowUp', openCheats: 'F13', cycleHotbar: 'KeyV' }
read('all-changed', changed)
for (let mask = 0; mask < 16; mask += 1) {
  const value = structuredClone(changed)
  if (mask & 1) delete value.enhancedEffects
  if (mask & 2) delete value.reverseHotbarCycle
  if (mask & 4) delete value.reducedScreenFlashes
  if (mask & 8) for (const key of ['enableActivityMessages', 'enableGlobalChat', 'enableOnlineFeatures', 'enableSharedHub', 'submitRunsToServer']) delete value[key]
  read('historical-key-families-' + mask, value)
}
for (const key of Object.keys(changed)) {
  for (const value of [null, 'yes', [], {}]) read('invalid-field-' + key + '-' + JSON.stringify(value), { ...changed, [key]: value })
  const missing = structuredClone(changed); delete missing[key]; read('missing-' + key, missing)
}
for (const [key, minimum, maximum] of [['cameraFovPercent',75,125], ['uiScalePercent',75,150], ['lightQualityPercent',24,100], ['musicVolumePercent',0,100], ['soundVolumePercent',0,100]]) {
  for (const value of [minimum - 1, minimum, maximum, maximum + 1, minimum + .5]) read('numeric-' + key + '-' + value, { ...changed, [key]: value })
}
read('extra-key', { ...changed, unexpected: true })
read('duplicate-json-field-last-wins', JSON.stringify(changed).replace('"musicVolumePercent":37', '"musicVolumePercent":12,"musicVolumePercent":37'), true)
for (const missing of [[], ['openCheats'], ['cycleHotbar'], ['openCheats', 'cycleHotbar']]) {
  for (const occupied of [[], ['Backquote'], ['KeyR','KeyV'], ['Backquote','KeyR','KeyV','F1','F2']]) {
    const value = fresh()
    for (const key of missing) delete value.controls[key]
    for (const [index, code] of occupied.entries()) value.controls[settings.GAME_BINDING_ACTIONS[index]] = code
    read('control-migration-' + missing.join('-') + '-' + occupied.join('-'), value)
  }
}
for (const value of ['', 'x'.repeat(64), 'x'.repeat(65), '😀'.repeat(32), '😀'.repeat(33), 4, false, null]) {
  const input = fresh(); input.controls.moveUp = value; read('control-length-type-' + JSON.stringify(value), input)
}
for (const key of settings.GAME_BINDING_ACTIONS) {
  const input = fresh(); delete input.controls[key]; read('missing-control-' + key, input)
}
read('duplicate-controls', { ...fresh(), controls: { ...fresh().controls, moveUp: 'KeyS' } })
read('extra-controls-key', { ...fresh(), controls: { ...fresh().controls, invented: 'F24' } })

const writes = []
function write(name, value) {
  let serialized
  const normalized = settings.setGameSettings(value, { getItem: () => null, setItem: (key, text) => { serialized = text } })
  writes.push({ name, input: value, normalized, serialized })
}
write('default', fresh())
write('all-changed', changed)
for (const key of ['cameraFovPercent','uiScalePercent','lightQualityPercent','musicVolumePercent','soundVolumePercent']) {
  for (const value of [0, 1, 74, 100, 150, 65535]) write('bounded-' + key + '-' + value, { ...changed, [key]: value })
}
for (const code of ['', 'x'.repeat(65), '😀'.repeat(33), 'KeyS']) write('normalized-control-' + code.length, { ...changed, controls: { ...changed.controls, moveUp: code } })
writeFixture('browser-settings-goldens.json', 'browser-settings-goldens.mjs',
  'Current Website fa08ae249ff5563570bdb2a653e2ef9358ce7b83 game-settings.ts readGameSettings/setGameSettings, unchanged storage and normalization functions',
  { storageKey: settings.GAME_SETTINGS_STORAGE_KEY, reads, writes })
