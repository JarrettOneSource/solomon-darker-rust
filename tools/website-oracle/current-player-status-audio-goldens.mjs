#!/usr/bin/env node
// Source receivers own one local charm stream and four ordered spatial rescue sounds.
import { execFileSync } from 'node:child_process'
import { loadKernel, websiteDir, writeFixture } from './oracle.mjs'
const native = await loadKernel('game-audio-native.ts')
const { subscribeRunCharmLossAudio } = await loadKernel('run-charm-audio.ts')
const sourceRevision = execFileSync('git', ['rev-parse', 'HEAD'], { cwd: websiteDir, encoding: 'utf8' }).trim()
const cases = []
for (const gain of [0, .375, 1]) {
  let listener
  const heard = []
  const audio = {
    playStream: (cue, options) => heard.push({ action: 'play', cue, playbackRate: options.playbackRate, volume: options.volume }),
    stopStream: cue => heard.push({ action: 'stop', cue }),
  }
  const dispose = subscribeRunCharmLossAudio(audio, 'owner', 'run', callback => { listener = callback; return () => { listener = null } })
  const frames = []
  for (const [index, type, targetPlayerId] of [[1, 'player-charm-lost', 'owner'], [2, 'player-charm-lost', 'peer'],
    [3, 'player-cheat-death', 'owner'], [4, 'player-cheat-death', 'peer']]) {
    const event = { actorId: 0, eventId: index, runId: 'run', sourcePosition: { x: 100, y: 100 }, targetPlayerId, tick: index, type }
    listener(event)
    if (event.type === 'player-cheat-death') for (const request of native.nativePlayerCheatDeathSoundRequests(gain)) {
      heard.push({ action: 'play', cue: request.cue, playbackRate: request.playbackRate, volume: request.volume })
    }
    frames.push({ event, expected: heard.splice(0) })
  }
  dispose()
  cases.push({ gain, frames, destroy: heard.splice(0) })
}
writeFixture('current-player-status-audio-goldens.json', 'tools/website-oracle/current-player-status-audio-goldens.mjs',
  'current run-charm-audio.ts and BoneyardScene.tsx player-cheat-death receiver/nativePlayerCheatDeathSoundRequests',
  { sourceRevision, cases })
