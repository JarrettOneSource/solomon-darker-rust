#!/usr/bin/env node
// Run the retained Pixi view without a GPU as well as its pure source plans. This preserves
// the first-snapshot cursor and the private dirt lifetime while the actual encounter moves.
import { registerHooks } from 'node:module'
import { readdirSync } from 'node:fs'
import { resolve } from 'node:path'
import { pathToFileURL } from 'node:url'
import { loadKernel, websiteDir, writeFixture } from './oracle.mjs'

// Supply Vite's asset URL table in Node; the source registration and view code stay intact.
const spriteGlobs = {}
for (const bank of ['deadhawg', 'bonedit']) {
  const directory = resolve(websiteDir, 'frontend/src/assets/game/boneyard', bank)
  spriteGlobs[`../assets/game/boneyard/${bank}/*.png`] = Object.fromEntries(
    readdirSync(directory).filter(name => name.endsWith('.png')).map(name => [
      `../assets/game/boneyard/${bank}/${name}`, pathToFileURL(resolve(directory, name)).href,
    ]),
  )
}
registerHooks({ load(url, context, nextLoad) {
  if (url.endsWith('.json')) return nextLoad(url, { ...context, importAttributes: { type: 'json' } })
  const loaded = nextLoad(url, context)
  if (url.endsWith('/editor/assets.ts')) return { ...loaded,
    source: `import.meta.glob = pattern => (${JSON.stringify(spriteGlobs)})[pattern];\n${loaded.source}` }
  return loaded
} })

const encounterKernel = await loadKernel('core-kernels/boneyard-encounter.ts')
const render = await loadKernel('renderer/boneyard-solomon-render.ts')
const { createNativeWorldManagerOrder } = await loadKernel('core-kernels/native-world-manager-order.ts')
const dirt = await loadKernel('renderer/boneyard-solomon-dirt-presentation.ts')
const { BoneyardSolomonView } = await loadKernel('renderer/boneyard-solomon-view.ts')
const { nativeSolomonSetPieceLighting } = await loadKernel('renderer/boneyard-lighting.ts')
const { NATIVE_GENERATED_BONEYARDS } = await loadKernel('host/native-generated-boneyards.ts')
const { Container, Texture } = await import(pathToFileURL(resolve(websiteDir, 'frontend/node_modules/pixi.js/lib/index.mjs')).href)
const dig = NATIVE_GENERATED_BONEYARDS[0].scene.solomonDig
const order = createNativeWorldManagerOrder({ nextRegistrationOrdinal: { actor: 40, transient: 70 } })
const lanternRegistration = order.register('actor')
const solomonRegistration = order.register('actor')
const textures = {
  lantern: Texture.EMPTY,
  solomonDig: Array(18).fill(Texture.EMPTY),
  solomonDialogueBody: Array(15).fill(Texture.EMPTY),
  solomonDialogueMouth: Array.from({ length: 3 }, () => Array(15).fill(Texture.EMPTY)),
  solomonWalk: Array.from({ length: 6 }, () => Array(15).fill(Texture.EMPTY)),
  solomonFlydirt: Texture.EMPTY,
  solomonGraveMark: Texture.EMPTY,
}

function capture(seed, mode, contact, paintEvery, initialPaintTick, totalTicks) {
  const options = mode === 'tutorial' ? { dialogueMode: 'tutorial', tutorialDialogueTicks: 400 } : {}
  let state = encounterKernel.createSolomonEncounter(dig, seed, options)
  const view = new BoneyardSolomonView({ scene: { solomonDig: dig } }, new Container(), textures)
  const records = []
  for (let tick = 0; tick <= totalTicks; tick++) {
    const priorPhase = state.phase
    const priorEventId = state.digEventId
    if (tick > 0) state = encounterKernel.stepSolomonEncounter(state, contact ? {
      local: { position: { x: dig.position.x + 30, y: dig.position.y + 50 } },
    } : {}, tick)
    if (tick < initialPaintTick || (tick - initialPaintTick) % paintEvery !== 0) continue
    view.update(state, tick, tick % 150 === 0 ? null : dig.lanternPosition)
    if (tick % 25 === 0 || tick === initialPaintTick || state.phase !== priorPhase || state.digEventId !== priorEventId || tick === totalTicks) {
      records.push({ tick, visual: render.boneyardSolomonVisualState(state, dig, tick), dirt: view.dirt,
        painters: render.boneyardSolomonPainterLayers(dig, state, lanternRegistration, solomonRegistration,
          tick % 150 === 0 ? null : dig.lanternPosition),
        dirtCount: view.dirtCount, dirtPassCount: view.dirtPassCount, frame: view.frame,
        bodyOffsetY: view.bodyOffsetY, clipRectWorld: view.clipRectWorld, graveMarkPassCount: view.graveMarkPassCount })
    }
  }
  view.destroy()
  return { seed, mode, contact, paintEvery, initialPaintTick, totalTicks,
    tutorialDialogueTicks: options.tutorialDialogueTicks ?? 0, records }
}

const scenarios = [
  capture('dig-without-contact', 'ordinary', false, 1, 0, 1800),
  capture('late-first-paint', 'ordinary', false, 7, 202, 1800),
  capture('ordinary-0', 'ordinary', true, 1, 0, 2200),
  capture('ordinary-3', 'ordinary', true, 3, 0, 2200),
  capture('tutorial', 'tutorial', true, 1, 0, 2200),
]
const headings = [-744, -372, -360, -348, -12.000001, -12, -11.999999, 0, 11.999999, 12, 12.000001,
  23.999999, 24, 347.999999, 348, 359.999999, 360, 372, 744]
const directions = headings.map(heading => ({ heading, direction: render.nativeSolomonDirection(heading) }))
const dirtStates = []
for (const position of [dig.position, { x: 0.125, y: -0.125 }, { x: 16384.125, y: 8192.0625 }]) {
  for (let age = 0; age <= 30; age++) dirtStates.push({ position, age, expected: dirt.nativeSolomonDirtStateAt(position, age) })
}
const events = [
  { id: 1, cue: 'shovel-1', tick: 2 }, { id: 2, cue: 'throw-dirt-1', tick: 5 },
  { id: 3, cue: 'throw-dirt-2', tick: 9 }, { id: 4, cue: 'shovel-2', tick: 11 },
]
const eventDeltas = [null, 0, 1, 2, 3, 4, 9].map(last => ({ last, events, expected: dirt.nativeSolomonDirtEventDelta(last, events) }))
const fallback = []
const fallbackView = new BoneyardSolomonView({ scene: { solomonDig: dig } }, new Container(), textures)
for (const tick of [0, 1, 5, 9, 55, 80, 200, 999]) {
  fallbackView.update(null, tick, null)
  fallback.push({ tick, frame: fallbackView.frame, offsetY: fallbackView.bodyOffsetY,
    clip: fallbackView.clipRectWorld, dirtCount: fallbackView.dirtCount, graveMarkPassCount: fallbackView.graveMarkPassCount })
}
fallbackView.destroy()
const lighting = nativeSolomonSetPieceLighting(dig.position, dig.lanternPosition, {
  acceptedSources: [], sourceIndicesAt: () => [], scalarAt: position => position.x === dig.lanternPosition.x ? 0.625 : 0.25,
})

writeFixture('solomon-presentation-goldens.json', 'tools/website-oracle/solomon-presentation-goldens.mjs',
  'Website/frontend/src/game/renderer/boneyard-solomon-{render,view,dirt-presentation}.ts; boneyard-lighting.ts',
  { dig, scenarios, directions, dirtStates, eventDeltas, fallback, lighting })
