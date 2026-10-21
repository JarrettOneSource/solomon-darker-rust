import { loadKernel, writeFixture } from './oracle.mjs'

// Exercise the public browser-input owner with native EventTargets. Only the
// presentation bank and physical devices are supplied; Source resolves each hold.
const { createBrowserGameplayInput } = await loadKernel('input/gameplay-input.ts')
const traces = []
const pad = (pressed) => ({
  index: 0, connected: true, mapping: 'standard', axes: [0, 0, 0, 0],
  buttons: Array.from({ length: 16 }, (_, index) => ({
    pressed: pressed && index === 2, value: pressed && index === 2 ? 1 : 0,
  })),
})
for (const initialBank of [0, 1, 2]) {
  const target = new EventTarget()
  const mouseTarget = new EventTarget()
  const visibilityTarget = Object.assign(new EventTarget(), { visibilityState: 'visible' })
  let bank = initialBank
  let gamepads = []
  const input = createBrowserGameplayInput({
    target, mouseTarget, visibilityTarget, getGamepads: () => gamepads,
    resolveQuickbarSlot: (slot) => bank * 8 + slot,
    onInput: () => {}, projectDirection: (point) => point, projectPointer: (point) => point,
  })
  const steps = []
  const step = (action) => {
    if (action.type === 'bank') bank = action.bank
    else if (action.type === 'gamepad') gamepads = [pad(action.pressed)]
    else if (action.type === 'touch') input.setTouchQuickbar(action.slot, action.pressed)
    else if (action.type === 'key') target.dispatchEvent(Object.assign(
      new Event(action.pressed ? 'keydown' : 'keyup', { cancelable: true }),
      { code: `Digit${action.slot}`, repeat: false },
    ))
    else if (action.type === 'mouse') (action.pressed ? mouseTarget : target).dispatchEvent(
      Object.assign(new Event(action.pressed ? 'mousedown' : 'mouseup', { cancelable: true }),
        { button: 2, clientX: 800, clientY: 450 }),
    )
    steps.push({ ...action, expected: input.sample().input.cast.quickbar })
  }
  step({ type: 'key', slot: 2, pressed: true })
  step({ type: 'bank', bank: (initialBank + 1) % 3 })
  step({ type: 'key', slot: 2, pressed: true })
  step({ type: 'key', slot: 2, pressed: false })
  step({ type: 'key', slot: 2, pressed: true })
  step({ type: 'mouse', pressed: true })
  step({ type: 'bank', bank: (initialBank + 2) % 3 })
  step({ type: 'key', slot: 2, pressed: false })
  step({ type: 'mouse', pressed: false })
  step({ type: 'mouse', pressed: true })
  step({ type: 'touch', slot: 5, pressed: true })
  step({ type: 'bank', bank: initialBank })
  step({ type: 'touch', slot: 5, pressed: true })
  step({ type: 'touch', slot: 5, pressed: false })
  step({ type: 'mouse', pressed: false })
  step({ type: 'gamepad', pressed: true })
  step({ type: 'bank', bank: (initialBank + 1) % 3 })
  step({ type: 'gamepad', pressed: true })
  step({ type: 'gamepad', pressed: false })
  step({ type: 'gamepad', pressed: true })
  step({ type: 'gamepad', pressed: false })
  traces.push({ initialBank, steps })
  input.destroy()
}
writeFixture('hotbar-input-goldens.json', 'hotbar-input-goldens.mjs',
  'Current Website fa08ae249ff5563570bdb2a653e2ef9358ce7b83 input/gameplay-input.ts public createBrowserGameplayInput',
  { traces })
