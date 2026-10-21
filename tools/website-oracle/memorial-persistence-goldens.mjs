// The actual durable Memorial reader validates retained source states, rather than
// accepting a second loosely shaped persistence model.
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { loadKernel, writeFixture } from './oracle.mjs'

const { openGameMemorialPersistence } = await loadKernel('host/game-memorial-persistence.ts')
const { archiveHubMemorialPortrait } = await loadKernel('core-kernels/hub-memorial.ts')
const directory = mkdtempSync(join(tmpdir(), 'rust-memorial-source-'))
const path = join(directory, 'memorial.json')
const copy = value => structuredClone(value)
const cases = []

try {
  const file = openGameMemorialPersistence(path)
  const initial = file.initialState
  let completed = initial
  for (let index = 0; index < 13; index += 1) {
    completed = archiveHubMemorialPortrait(completed, {
      accountUsername: index % 2 ? null : 'MemorialAccount', awesomeness: 4567 + index,
      awesomestKill: 'Horned Skeleton Fire Archer', capturedAtTick: 30000 + index,
      config: { discipline: 'arcane', displayName: `Aurelia ${index}`, element: 'earth' },
      elapsedTicks: 12345, equipment: { hat: null, robe: null, weapon: null },
      headingIndex: 12, level: 7, monstersKilled: 321, playerId: `player-${index}`,
      portraitScale: .925, runId: `run-${index}`, wave: 12,
    }, index % 5)
  }
  file.persist(completed)
  const document = JSON.parse(readFileSync(path, 'utf8'))
  const casesToRead = [
    ['stock', { state: initial }], ['retained-ten', document],
    ['extra-envelope-key', { state: completed, operatorNote: 'retained' }],
    ['not-object', []], ['missing-state', {}], ['missing-next-age', { state: {} }],
    ['state-unknown-key', { state: { ...initial, unknown: true } }],
    ['too-few-slots', { state: { ...initial, slots: initial.slots.slice(1) } }],
    ['next-age-before-stock', { state: { ...initial, nextAge: 1000 } }],
    ['next-portrait-ring', { state: { ...completed, nextPortraitId: 109 } }],
  ]
  const mutate = (name, source, change) => {
    const value = copy(source)
    change(value.state)
    casesToRead.push([name, value])
  }
  mutate('stock-age', { state: initial }, state => { state.slots[0].age = 100 })
  mutate('stock-marker', { state: initial }, state => { state.slots[0].marker = true })
  mutate('stock-portrait-id', { state: initial }, state => { state.slots[0].portraitId = 109 })
  mutate('duplicate-age', document, state => { state.slots[1].age = state.slots[0].age })
  mutate('duplicate-portrait-id', document, state => { state.slots[1].portraitId = state.slots[0].portraitId })
  mutate('duplicate-participant', document, state => { state.slots[1].portrait = copy(state.slots[0].portrait) })
  mutate('oldest-retained-age', document, state => { state.slots[0].age = 7 })
  mutate('portrait-scale-low', document, state => { state.slots[0].portrait.portraitScale = .1 })
  mutate('portrait-scale-high', document, state => { state.slots[0].portrait.portraitScale = 1.01 })
  mutate('heading-outside-native', document, state => { state.slots[0].portrait.headingIndex = 24 })
  mutate('captured-tick-source-integer', document, state => { state.slots[0].portrait.capturedAtTick = 1e100 })
  mutate('portrait-unknown-key', document, state => { state.slots[0].portrait.guessed = true })
  mutate('equipment-selector', document, state => {
    state.slots[0].portrait.equipment.hat = { primaryTint: 0xffffff, secondaryTint: 0, selector: 4 }
  })
  mutate('equipment-tint', document, state => {
    state.slots[0].portrait.equipment.robe = { primaryTint: 0x1000000, secondaryTint: 0, selector: 2 }
  })
  mutate('equipment-weapon-kind', document, state => {
    state.slots[0].portrait.equipment.weapon = { kind: 'sword', selector: 0 }
  })
  mutate('valid-equipment-and-unicode', document, state => {
    const portrait = state.slots[0].portrait
    portrait.playerId = '\u2728\u00e9-wizard'
    portrait.config.displayName = '\ud83e\uddd9'.repeat(32)
    portrait.equipment = {
      hat: { primaryTint: 0xffffff, secondaryTint: 0, selector: 3 },
      robe: { primaryTint: 0, secondaryTint: 0xffffff, selector: 2 },
      weapon: { kind: 'wand', selector: 5 },
    }
  })
  mutate('missing-nullable-portrait', { state: initial }, state => { delete state.slots[0].portrait })
  mutate('missing-nullable-account', document, state => { delete state.slots[0].portrait.accountUsername })
  mutate('missing-nullable-equipment', document, state => { delete state.slots[0].portrait.equipment.hat })
  mutate('reserved-player-id', document, state => { state.slots[0].portrait.playerId = '__proto__' })
  mutate('utf16-display-name-limit', document, state => { state.slots[0].portrait.config.displayName = '\ud83e\uddd9'.repeat(33) })
  mutate('fractional-captured-tick', document, state => { state.slots[0].portrait.capturedAtTick = 1.5 })
  mutate('negative-captured-tick', document, state => { state.slots[0].portrait.capturedAtTick = -1 })
  mutate('negative-zero-captured-tick', document, state => { state.slots[0].portrait.capturedAtTick = -0 })
  mutate('wrong-slot-marker-type', document, state => { state.slots[0].marker = 1 })
  mutate('wrong-integer-type', document, state => { state.slots[0].portrait.level = '7' })
  mutate('huge-age-does-not-narrow', document, state => { state.slots[0].age = 1e100 })
  mutate('completion-count-with-stock-slots', { state: initial }, state => { state.nextAge += 1 })
  mutate('unknown-discipline', document, state => { state.slots[0].portrait.config.discipline = 'warrior' })
  mutate('unknown-element', document, state => { state.slots[0].portrait.config.element = 'ice' })
  mutate('portrait-scale-zero', document, state => { state.slots[0].portrait.portraitScale = 0 })
  mutate('portrait-id-fraction', document, state => { state.slots[0].portraitId += .5 })
  mutate('next-age-zero', { state: initial }, state => { state.nextAge = 0 })
  mutate('slots-wrong-type', { state: initial }, state => { state.slots = {} })
  mutate('equipment-unknown-key', document, state => { state.slots[0].portrait.equipment.unknown = 1 })
  mutate('garment-unknown-key', document, state => {
    state.slots[0].portrait.equipment.hat = { primaryTint: 0, secondaryTint: 0, selector: 0, unknown: 1 }
  })
  mutate('weapon-selector', document, state => { state.slots[0].portrait.equipment.weapon = { kind: 'staff', selector: 6 } })
  mutate('account-empty', document, state => { state.slots[0].portrait.accountUsername = '' })
  mutate('elapsed-upper-bound', document, state => { state.slots[0].portrait.elapsedTicks = 60_480_001 })
  for (const [name, value] of casesToRead) {
    writeFileSync(path, JSON.stringify(value))
    try {
      const reader = openGameMemorialPersistence(path)
      const state = reader.initialState
      reader.persist(state)
      cases.push({ name, document: value, accepted: true, state,
        encoded: readFileSync(path, 'utf8').slice(0, -1) })
    } catch (error) {
      cases.push({ name, document: value, accepted: false, error: error.message })
    }
  }
  writeFixture('memorial-persistence-goldens.json', 'memorial-persistence-goldens.mjs',
    'host/game-memorial-persistence.ts + protocol/codecs/hub.ts', { cases })
} finally {
  rmSync(directory, { recursive: true, force: true })
}
