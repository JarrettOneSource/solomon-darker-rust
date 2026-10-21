import { loadKernel, writeFixture } from './oracle.mjs'

const hall = await loadKernel('hall-of-fame-store.ts')
const legacyKey = 'sdr.game.hall-of-fame.v2'
function entry(runId = 'run', awesomeness = 10) {
  return {
    accountUsername: null,
    awesomeness,
    awesomestKill: null,
    completedAtUtc: '2026-10-21T00:00:00.000Z',
    discipline: 'mind',
    elapsedTicks: 1000,
    element: 'fire',
    headingIndex: 4,
    highestSkills: [],
    level: 1,
    monstersKilled: 0,
    perksUsed: [],
    portraitScale: 1,
    runId,
    wave: 1,
    wizardName: 'Helvidius',
  }
}
function storage(current, legacy) {
  const values = new Map([[hall.HALL_OF_FAME_STORAGE_KEY, current], [legacyKey, legacy]])
  return {
    getItem: key => values.get(key) ?? null,
    setItem: (key, value) => values.set(key, value),
  }
}
const reads = []
function read(name, current, legacy = null) {
  reads.push({ name, current, legacy, expected: hall.readLocalHallOfFame(storage(current, legacy)) })
}
function readRows(name, rows, legacy = null) { read(name, JSON.stringify(rows), legacy) }
read('absent', null)
for (const raw of ['', '{', 'null', 'true', '1', '{}', '"text"', '[null]', '[[]]']) {
  read('malformed-key-' + raw, raw)
  read('malformed-modern-rescues-legacy-' + raw, raw, JSON.stringify([entry('legacy')]))
}
read('legacy-only', null, JSON.stringify([entry('legacy')]))
readRows('malformed-legacy-preserves-modern', [entry()], '{')
readRows('modern-wins-before-ranking', [entry('duplicate', 3), entry('modern', 10)], JSON.stringify([entry('duplicate', 99), entry('late', 8)]))
readRows('first-modern-duplicate-wins', [entry('duplicate', 3), entry('duplicate', 99), entry('other', 10)])
readRows('legacy-first-duplicate-wins', [], JSON.stringify([entry('duplicate', 3), entry('duplicate', 99)]))
readRows('stable-top-100-after-dedup', Array.from({length:70}, (_, index) => entry('modern-' + index)), JSON.stringify([
  ...Array.from({length:50}, (_, index) => entry('modern-' + index, 99)),
  ...Array.from({length:50}, (_, index) => entry('legacy-' + index)),
]))
for (const discipline of ['arcane','body','mind','air','earth','ether','fire','water']) {
  for (const element of ['air','earth','ether','fire','water']) {
    readRows('identity-' + discipline + '-' + element, [{...entry(), discipline, element}])
  }
}
for (const key of Object.keys(entry())) {
  const missing = entry(); delete missing[key]
  readRows('missing-' + key, [missing])
  for (const invalid of [false, {}, [], 1.5]) {
    readRows('invalid-type-' + key + '-' + JSON.stringify(invalid), [{...entry(), [key]: invalid}])
  }
}
for (const key of ['awesomeness','elapsedTicks','headingIndex','level','monstersKilled','wave']) {
  for (const number of [-1, 0, 0.5, 4294967296, Number.MAX_SAFE_INTEGER, Number.MAX_SAFE_INTEGER + 1]) {
    readRows('integer-' + key + '-' + number, [{...entry(), [key]: number}])
  }
}
for (const field of ['rank', 'skillId']) {
  for (const number of [-1, 0.5, 4294967296, Number.MAX_SAFE_INTEGER, Number.MAX_SAFE_INTEGER + 1]) {
    readRows('skill-' + field + '-' + number, [{...entry(), highestSkills: [{rank:1, skillId:8, [field]:number}]}])
  }
}
for (const number of [-1, 0.5, 4294967296, Number.MAX_SAFE_INTEGER, Number.MAX_SAFE_INTEGER + 1]) {
  readRows('perk-' + number, [{...entry(), perksUsed:[number]}])
}
for (const length of [0, 3, 4]) readRows('skill-count-' + length, [{...entry(), highestSkills:Array.from({length}, () => ({rank:1, skillId:8}))}])
for (const length of [0, 9, 10]) readRows('perk-count-' + length, [{...entry(), perksUsed:Array.from({length}, () => 0)}])
for (const value of [0, .849999999999, .85, .975, 1, 1.000000000001]) readRows('portrait-' + value, [{...entry(), portraitScale:value}])
for (const discipline of ['', 'unknown', 'ARCANE']) readRows('unknown-root-' + discipline, [{...entry(), discipline}])
for (const element of ['', 'unknown', 'FIRE']) readRows('unknown-element-' + element, [{...entry(), element}])
readRows('bad-row-invalidates-whole-key', [entry('good'), {...entry('bad'), wave:-1}], JSON.stringify([entry('legacy')]))
readRows('extra-fields-normalized-away', [{...entry(), extra:true, highestSkills:[{rank:1, skillId:8, extra:'ignored'}]}])
readRows('strings-are-not-normalized', [{...entry(''), accountUsername:'', awesomestKill:'', completedAtUtc:'not-a-date', wizardName:'😀\u0000'}])

const writes = []
function write(name, next, current, legacy = null) {
  const memory = storage(current, legacy)
  const expected = hall.recordLocalHallOfFame(next, memory)
  writes.push({name, next, current, legacy, expected, serialized:memory.getItem(hall.HALL_OF_FAME_STORAGE_KEY), legacyAfter:memory.getItem(legacyKey)})
}
write('first-record', entry(), null)
write('replace-same-run', entry('same', 1), JSON.stringify([entry('same', 99),entry('other', 2)]))
write('new-entry-first-on-tie', entry('new'), JSON.stringify([entry('first'),entry('second')]))
write('legacy-preserved', {...entry('mixed'), discipline:'earth'}, null, JSON.stringify([entry('legacy'),entry('mixed', 99)]))
write('bad-modern-legacy-rescue', entry('new'), '{', JSON.stringify([entry('legacy')]))
write('top-100', entry('new', 1000), JSON.stringify(Array.from({length:105}, (_, index) => entry('existing-' + index, index))))
write('complete-safe-integer-row', {
  ...entry('safe'), headingIndex:Number.MAX_SAFE_INTEGER, level:Number.MAX_SAFE_INTEGER,
  highestSkills:[{rank:Number.MAX_SAFE_INTEGER,skillId:Number.MAX_SAFE_INTEGER}],
  perksUsed:[Number.MAX_SAFE_INTEGER], portraitScale:.85,
}, null)
writeFixture('browser-hall-goldens.json', 'browser-hall-goldens.mjs',
  'Current Website fa08ae249ff5563570bdb2a653e2ef9358ce7b83 hall-of-fame-store.ts unchanged readLocalHallOfFame/recordLocalHallOfFame, v3/v2 independent validation and stable top-100 merge',
  {storageKey:hall.HALL_OF_FAME_STORAGE_KEY, legacyStorageKey:legacyKey, reads, writes})
