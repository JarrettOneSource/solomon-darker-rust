import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { loadKernel, websiteDir, writeFixture } from './oracle.mjs'

const create = await loadKernel('create-menu-motion.ts')
const title = await loadKernel('renderer/title-menu-render-contract.ts')
const pause = await loadKernel('pause-menu-contract.ts')
const navigation = await loadKernel('input/gamepad-menu-navigation.ts')
const settings = await loadKernel('game-settings.ts')
const times = [0, 10, 200, 600, 610, 900, 910, 970, 980, 1200, 1310, 1320, 1340, 1400, 1610, 1640, 1680, 2330, 2630]
const createCases = times.map(elapsed => ({
  elapsed,
  entry: create.createEntryMotionAt(elapsed),
  selection: create.createSelectionMotionAt(elapsed),
  idle: create.createHandIdleOffsetAt(elapsed),
  elements: ['air', 'earth', 'ether', 'fire', 'water'].map(element => ({
    element,
    reveal: create.createElementRevealMotionAt(element, elapsed),
    selected: create.createSelectedElementMotionAt(element, elapsed),
  })),
  disciplines: ['arcane', 'body', 'mind'].map(discipline => ({
    discipline, position: create.createDisciplineRevealMotionAt(discipline, elapsed),
  })),
}))
const manifest = JSON.parse(readFileSync(resolve(websiteDir, 'frontend/src/assets/game/native-ui-assets.json'), 'utf8'))
const widths = Array.from({length: 9}, (_, i) => manifest.atlases.Title.records[i + 16].frame[2])
const simulation = title.createTitleGraveRows(widths)
const titleCases = []
let tick = 0
for (const target of [0, 1, 60, 600]) {
  while (tick < target) {
    for (const row of simulation.rows) title.stepTitleGraveRow(widths, row, simulation.random)
    tick += 1
  }
  titleCases.push({ ticks: tick, rows: structuredClone(simulation.rows), offsets: title.titleBackdropOffsetsAt(tick / 60) })
}
const pauseCases = [0, 10, 100, 190, 200, 280, 290, 400].map(elapsed => ({
  elapsed, opening: pause.nativePauseMenuReveal('opening', elapsed), closing: pause.nativePauseMenuReveal('closing', elapsed),
}))
const navPads = []
const pad = (x, y, buttons) => ({ connected: true, mapping: 'standard', index: 0, axes: [x, y],
  buttons: Array.from({length: 16}, (_, index) => ({pressed: buttons.includes(index), value: 0})) })
for (const x of [-0.63, 0, 0.63]) for (const y of [-0.63, 0, 0.63]) {
  navPads.push({ x, y, buttons: [], state: navigation.readMenuGamepad([pad(x, y, [])]) })
}
for (const button of [0, 1, 4, 5, 12, 13, 14, 15]) {
  navPads.push({ x: 0, y: 0, buttons: [button], state: navigation.readMenuGamepad([pad(0, 0, [button])]) })
}
const repeats = [[null, 'up', 0, 0], ['up', 'up', 10, 320], ['up', 'up', 320, 320], ['up', 'left', 330, 430], ['up', null, 400, 430]]
  .map(args => ({ args, value: navigation.advanceMenuDirectionRepeat(...args) }))
const centers = [[0, 0], [30, 20], [0, 100], [-30, -20], [100, 0]]
const candidates = centers.map(([x, y], value) => ({value, bounds: {left:x-5,top:y-5,width:10,height:10,right:x+5,bottom:y+5}}))
const spatial = ['up', 'down', 'left', 'right'].map(direction => ({ direction,
  selected: navigation.chooseSpatialTarget(candidates[0], candidates, direction) }))
// Reuse Source's complete public action order, including console/hotbar cycling.
// Named inputs preserve the meaning of the belt scenario when that order changes.
const bindingKeys = settings.GAME_BINDING_ACTIONS
const bindings = [['moveUp', 'KeyS'], ['belt1', 'KeyW'], ['openMenu', 'F2'], ['openChat', 'KeyI'], ['openInventory', 'KeyT'], ['moveUp', ''], ['openCheats', 'KeyI']].map(([key, code]) => {
  const value = settings.rebindGameControl(settings.DEFAULT_GAME_CONTROL_BINDINGS, key, code)
  return {action: bindingKeys.indexOf(key), code, values: bindingKeys.map(key => value[key])}
})
writeFixture('menu-goldens.json', 'tools/website-oracle/menu-goldens.mjs',
  ['create-menu-motion.ts', 'renderer/title-menu-render-contract.ts', 'pause-menu-contract.ts'],
  { createCases, titleWidths: widths, titleCases, pauseCases, navPads, repeats, centers, spatial, bindings })
