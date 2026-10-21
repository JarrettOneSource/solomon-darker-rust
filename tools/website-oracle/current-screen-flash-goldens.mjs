#!/usr/bin/env node
import { execFileSync } from 'node:child_process'
import { loadKernel, websiteDir, writeFixture } from './oracle.mjs'

const screen = await loadKernel('core-kernels/native-screen-flash.ts')
const sourceRevision = execFileSync('git', ['rev-parse', 'HEAD'], { cwd: websiteDir, encoding: 'utf8' }).trim()
const flash = { red: .75, green: .25, blue: 1, alpha: .5, decayPerTick: .01, pointAttenuated: true }
const input = (tick, worldKey, onlyIfClear) => ({ tick, worldKey, onlyIfClear, position: { x: tick + .25, y: -17 }, flash })
const operations = [
  { tick: 1, writes: [input(1, 'boneyard:a', false), input(1, 'boneyard:a', true),
    input(1, 'boneyard:b', false), input(1, 'boneyard:b', true)] },
  { tick: 50, writes: [input(50, 'boneyard:a', false)] },
  { tick: 100, writes: [input(100, 'boneyard:a', true)] },
  { tick: 250, writes: [] },
  { tick: 251, writes: [] },
  { tick: 270, writes: [input(270, 'boneyard:a', false), input(270, 'boneyard:b', true)] },
  { tick: 470, writes: [] },
  { tick: 471, writes: [] },
  { tick: 500, writes: [] },
  { tick: 501, reset: true, writes: [] },
  { tick: 502, writes: [input(502, 'hub:courtyard', true)] },
  { tick: 1000, writes: [] },
  { tick: 1001, writes: [input(1001, 'hub:courtyard', false)] },
  { tick: 1202, writes: [] },
]
let state = screen.createNativeScreenFlashes()
const frames = []
for (const operation of operations) {
  if (operation.reset) state = screen.resetNativeScreenFlashes(state)
  const writer = screen.createNativeScreenFlashWriter(state, operation.tick)
  for (const write of operation.writes) writer.write(write)
  state = writer.state()
  frames.push({ ...operation, expected: state })
}
writeFixture('current-screen-flash-goldens.json', 'current-screen-flash-goldens.mjs',
  'core-kernels/native-screen-flash.ts', { sourceRevision, frames })
