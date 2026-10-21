import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { gameDir, loadKernel, writeFixture } from './oracle.mjs'

const { joystickVector } = await loadKernel('input/movement-input.ts')
const source = readFileSync(resolve(gameDir, 'input/TouchJoystick.tsx'), 'utf8')
const start = source.indexOf('    const rotation = mobileUi.customized')
const end = source.indexOf('    inputSinkRef.current(movement)', start)
if (start < 0 || end < start) throw new Error('TouchJoystick.update source boundary changed')

// Execute the actual layout-dependent callback body with its borrowed DOM measurements.
// React owns those measurements, but the input/knob calculation has no React dependency.
const update = new Function(
  'bounds', 'base', 'mobileUi', 'mobileUiId', 'clientX', 'clientY',
  'joystickVector', 'setKnobOffset', `${source.slice(start, end)}\nreturn movement`,
)
const cases = []
for (const customized of [false, true]) {
  for (const rotation of [-180, -135, -90, -45, 0, 22.5, 45, 90, 180]) {
    for (const scale of [0.4, 1, 2.7]) {
      const radians = (customized ? rotation : 0) * Math.PI / 180
      const expansion = Math.abs(Math.cos(radians)) + Math.abs(Math.sin(radians))
      const width = 237.5 * scale * expansion
      const height = width
      const bounds = { left: 300 - width / 2, top: 200 - height / 2, width, height }
      for (const offset of [[0, 0], [5, 0], [40, 0], [0, -40], [200, 100]]) {
        const pointer = { x: 300 + offset[0], y: 200 + offset[1] }
        let knobOffset
        const movement = update(
          bounds,
          { offsetWidth: 238, offsetHeight: 238 },
          { customized, layout: { leftJoystick: { rotation } } },
          'leftJoystick', pointer.x, pointer.y, joystickVector,
          value => { knobOffset = value },
        )
        cases.push({ customized, rotation, bounds, pointer, movement, knobOffset })
      }
    }
  }
}
writeFixture(
  'mobile-joystick-goldens.json',
  'tools/website-oracle/mobile-joystick-goldens.mjs',
  'Website/frontend/src/game/input/TouchJoystick.tsx:update',
  { cases },
)
