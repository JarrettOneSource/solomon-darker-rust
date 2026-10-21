#!/usr/bin/env node
import { execFileSync } from 'node:child_process'
import { loadKernel, websiteDir, writeFixture } from './oracle.mjs'

const player = await loadKernel('core-kernels/player-character.ts')
const sourceRevision = execFileSync('git', ['rev-parse', 'HEAD'], { cwd: websiteDir, encoding: 'utf8' }).trim()
const facings = [-.01, 0, 7.4999999, 7.5, 7.5000001, 103.25, 172.4999999, 172.5,
  352.4999999, 352.5, 359.99999, 360, 721.1].map(heading => ({ heading, ...player.playerCharacterFacing(heading) }))
let state = player.createPlayerCharacter({ displayName: 'Heading', discipline: 'mind', element: 'ether' }, { x: 0, y: 0 })
const frames = []
for (let tick = 1; tick <= 90; tick += 1) {
  const movement = tick < 30 ? { x: .3, y: .7 } : tick < 60 ? { x: -.25, y: -.75 } : { x: 0, y: 0 }
  const casting = tick >= 20 && tick < 40
  state = { ...state, primaryCast: { ...state.primaryCast, actionTick: casting ? 3 : -1 } }
  const plan = player.planPlayerCharacterTick(state, { movement }, 1)
  if (tick === 25 || tick === 35) plan.face = true
  state = player.commitPlayerCharacterTick(state, plan, {
    x: state.position.x + plan.delta.x, y: state.position.y + plan.delta.y,
  })
  frames.push({ tick, movement, casting, face: plan.face === true,
    expected: [state.headingDegrees, state.headingIndex, state.position.x, state.position.y] })
}
writeFixture('current-player-heading-goldens.json', 'current-player-heading-goldens.mjs',
  'core-kernels/player-character.ts::playerCharacterFacing/commitPlayerCharacterTick', { sourceRevision, facings, frames })
