// Current Source snapshot admission: complete retained recipes and actual limits.
import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { fixturesDir, loadKernel, websiteDir, writeFixture } from './oracle.mjs'

const revision = execFileSync('git', ['-C', websiteDir, 'rev-parse', 'HEAD'], { encoding: 'utf8' }).trim()
assert.equal(revision, 'fa08ae249ff5563570bdb2a653e2ef9358ce7b83')
const codec = await loadKernel('protocol/codecs/flame-presentation.ts')
const protocol = await loadKernel('protocol/flame-presentation.ts')
const born = JSON.parse(readFileSync(resolve(fixturesDir, 'flame-birth-goldens.json'), 'utf8')).cases
const context = { worldKey: 'boneyard:flame', worldTick: 135, nextPrimaryId: 100 }
const frame = births => ({ kind: 'continuous', epoch: 1, worldKey: context.worldKey,
  throughTick: context.worldTick, afterTick: 132, births })
const ribbon = protocol.flamePresentationBirth(born[4].channel)
const source = protocol.flamePresentationBirth(born[4].source)
const inputs = []
function add(name, input, expectedContext = context) {
  inputs.push({ name, input, context: expectedContext })
}
function mutate(name, change, original = ribbon) {
  const input = frame([structuredClone(original)])
  change(input, input.births[0], input.births[0].recipe)
  add(name, input)
}
for (const row of born) {
  add(row.name, frame([protocol.flamePresentationBirth(row.channel), protocol.flamePresentationBirth(row.source)]))
}
for (const variant of ['endpoint', 'chain']) for (const enhanced of [true, false]) {
  const fade = structuredClone(source)
  fade.recipe.variant = variant
  fade.recipe.birth.birthAlpha = 1
  fade.recipe.alphaStep = Math.fround(enhanced ? .2 : .4)
  fade.recipe.baseScale = .5
  fade.recipe.wrapperScalar = .75
  fade.endTickExclusive = fade.birthTick + (enhanced ? 5 : 2)
  add(`${variant}-${enhanced}`, frame([fade]))
}
add('empty-reset', { kind: 'reset', epoch: 1, worldKey: context.worldKey, throughTick: 135, births: [] })
mutate('same-birth-pass-retirement', (_frame, birth) => { birth.endTickExclusive = birth.birthTick })
const hub = frame([structuredClone(ribbon)])
hub.worldKey = 'hub'
hub.births[0].worldKey = 'hub:courtyard'
add('hub-region-membership', hub, { ...context, worldKey: 'hub' })

mutate('epoch-must-be-positive', input => { input.epoch = 0 })
mutate('snapshot-world-membership', input => { input.worldKey = 'hub' })
mutate('stopped-world-clock', input => { input.throughTick -= 1 })
mutate('reversed-coverage', input => { input.afterTick = 136 })
mutate('id-must-be-allocated', (_frame, birth) => { birth.id = 100 })
mutate('reserved-owner', (_frame, birth) => { birth.ownerId = 'constructor' })
mutate('owner-utf16-limit', (_frame, birth) => { birth.ownerId = '😀'.repeat(65) })
mutate('actor-world-membership', (_frame, birth) => { birth.worldKey = 'hub:courtyard' })
mutate('kind-matches-recipe', (_frame, birth) => { birth.kind = 'fade' })
mutate('future-birth', (_frame, birth, recipe) => {
  birth.birthTick = 136
  birth.endTickExclusive = 137
  recipe.birth.ribbon.phaseDegrees = Math.fround(Math.imul(136, -3))
})
mutate('native-lifetime', (_frame, birth) => { birth.endTickExclusive = birth.birthTick + 2 })
mutate('expired-before-coverage', input => { input.afterTick = 134 })
mutate('duplicate-identity', input => { input.births.push(structuredClone(input.births[0])) })
mutate('suppressed-ribbon', (_frame, _birth, recipe) => { recipe.birth = { kind: 'legacy-suppressed' } })
mutate('main-needs-camera', (_frame, _birth, recipe) => { recipe.birth.cameraDisplacement = null })
mutate('camera-native-samples', (_frame, _birth, recipe) => { recipe.birth.cameraDisplacement = { x: .1, y: 0 } })
mutate('camera-native-magnitude', (_frame, _birth, recipe) => { recipe.birth.cameraDisplacement = { x: 4, y: 4 } })
mutate('role-width', (_frame, _birth, recipe) => { recipe.birth.width = .75 })
mutate('native-caller-offset', (_frame, _birth, recipe) => { recipe.birth.offset.x = 1 })
mutate('stored-coordinates', (_frame, _birth, recipe) => { recipe.birth.start.x = .1 })
mutate('ribbon-pair-budget', (_frame, _birth, recipe) => { recipe.birth.control = { x: 40000, y: 0 } })
mutate('ribbon-native-span', (_frame, _birth, recipe) => { recipe.birth.ribbon = { kind: 'short', widthFactor: 1 } })
mutate('phase-matches-birth-clock', (_frame, _birth, recipe) => { recipe.birth.ribbon.phaseDegrees += 1 })
mutate('path-intensity-native', (_frame, _birth, recipe) => { recipe.birth.path.intensity = .6 })
mutate('path-cardinality', (_frame, _birth, recipe) => { recipe.birth.path.radiusFactors.push(1) })
mutate('path-sample-range', (_frame, _birth, recipe) => { recipe.birth.path.radiusFactors[0] = .5 })
mutate('creator-lane', (_frame, _birth, recipe) => { recipe.birth.path.creatorRegistration.managerLane = 'transient' })
mutate('path-factory-ordinal', (_frame, _birth, recipe) => { recipe.birth.path.miscAppendOrdinal = 1 })
mutate('ribbon-painter-lane', (_frame, _birth, recipe) => { recipe.painterRegistration.managerLane = 'transient' })
mutate('safe-painter-ordinal', (_frame, _birth, recipe) => { recipe.painterRegistration.registrationOrdinal = 2 ** 53 })
mutate('fade-painter-lane', (_frame, _birth, recipe) => { recipe.painterRegistration.managerLane = 'actor' }, source)
mutate('fade-native-clock', (_frame, _birth, recipe) => { recipe.birth.birthAlpha = 1 }, source)
mutate('fade-native-alpha-step', (_frame, _birth, recipe) => { recipe.alphaStep = .2 }, source)
mutate('fade-scale-range', (_frame, _birth, recipe) => { recipe.baseScale = .5 }, source)
mutate('fade-stored-scale', (_frame, _birth, recipe) => { recipe.baseScale = .9 }, source)
mutate('fade-source-wrapper', (_frame, _birth, recipe) => { recipe.wrapperScalar = 1.25 }, source)
mutate('extra-native-coordinate', (_frame, _birth, recipe) => { recipe.birth.start.z = 1 })
mutate('missing-camera-contract', (_frame, _birth, recipe) => { delete recipe.birth.cameraDisplacement })
const weak = protocol.flamePresentationBirth(born[1].channel)
mutate('missing-null-camera-contract', (_frame, _birth, recipe) => { delete recipe.birth.cameraDisplacement }, weak)

const cases = inputs.map(row => {
  try {
    const decoded = codec.flamePresentationFrame(row.input, 'snapshot.flamePresentation',
      row.context.worldKey, row.context.worldTick, row.context.nextPrimaryId)
    assert.deepEqual(decoded, row.input)
    return { ...row, accepted: true }
  } catch (error) {
    if (error.name !== 'GameProtocolError') throw error
    return { ...row, accepted: false, error: error.message }
  }
})
writeFixture('flame-protocol-goldens.json', 'tools/website-oracle/flame-protocol-goldens.mjs',
  `Website@${revision}/protocol/codecs/{flame-presentation,weld-flame}.ts`, { cases })
