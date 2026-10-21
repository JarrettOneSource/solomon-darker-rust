// All keyboard combinations, radial boundaries, d-pad priority and input-device priority.
import { loadKernel, point, writeFixture } from './oracle.mjs'

const input = await loadKernel('input/movement-input.ts')
const sticks = []
for (const x of [-1, -0.95, -0.5, -0.2, 0, 0.2, 0.5, 0.95, 1]) {
  for (const y of [-1, -0.5, 0, 0.2, 1]) {
    sticks.push({ input: [x, y], movement: point(input.radialDeadZone(x, y, 0.2, 0.95)) })
  }
}
const keyboard = []
for (let mask = 0; mask < 16; mask += 1) {
  const state = input.createMovementInputState()
  for (const [bit, code] of ['KeyA', 'KeyD', 'KeyW', 'KeyS'].entries()) {
    if (mask & (1 << bit)) state.press(code)
  }
  const sample = state.sample()
  keyboard.push({ mask, device: sample.device, movement: point(sample.movement) })
}
const gamepads = [
  { connected: true, mapping: 'standard', axes: [1, 0], index: 0 },
  { connected: false, mapping: 'standard', axes: [1, 0], index: 0 },
  { connected: true, mapping: '', axes: [1, 0], index: 0 },
  { connected: true, mapping: 'standard', axes: [-1, 0], index: 0,
    buttons: Array.from({length: 16}, (_, i) => ({pressed: i === 13 || i === 15, value: 0})) },
].map(gamepad => ({ gamepad, movement: point(input.movementFromGamepads([gamepad])) }))
const touch = [[0, 0], [0.25, 0.5], [1, 1], [-2, 0]].map(movement => ({
  input: movement, movement: point(input.normalizeMovement({x: movement[0], y: movement[1]})),
}))
const priorities = [[0, 0], [0.5, 0], [0.0005, 0]].map(touch => {
  const state = input.createMovementInputState()
  state.press('KeyA')
  state.setTouch({ x: touch[0], y: touch[1] })
  const sample = state.sample([{ connected: true, mapping: 'standard', axes: [0, 1], index: 0 }])
  return { touch, device: sample.device, movement: point(sample.movement) }
})
writeFixture('movement-input-goldens.json', 'tools/website-oracle/movement-input-goldens.mjs',
  ['input/movement-input.ts'], { sticks, keyboard, gamepads, touch, priorities })
