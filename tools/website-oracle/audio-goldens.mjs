import { loadKernel, writeFixture } from './oracle.mjs'

const audio = await loadKernel('game-audio-native.ts')
const boundaries = [0, 199, 200, 980, 990, 1340, 1640, 3000]
const intervals = boundaries.map((current, index) => [index ? boundaries[index - 1] : -1, current])
intervals.push([-1, 3000], [1340, 1340], [1640, 2000])
const elements = ['air', 'earth', 'ether', 'fire', 'water']
const entry = intervals.map(([previous, current]) => ({
  previous, current, expected: audio.createEntryAudioEvents(previous, current),
}))
const selection = elements.flatMap(element => intervals.map(([previous, current]) => ({
  element, previous, current, expected: audio.createSelectionAudioEvents(element, previous, current),
})))
const attenuation = [-1, 0, 149, 150, 300, 637.5, 799, 800, 801, 2000].map(distance => ({
  distance, expected: audio.hubAudioAttenuation(distance),
}))
const steps = ['local-player', 'wizard-🧙', '', 'second-player'].flatMap(playerId =>
  [0, 1, 2, 17, 10000, 0xffffffff].map(tick => ({
    playerId, tick, expected: audio.nativeFootstepCue(tick, playerId),
  })))
const teacher = [0, 1, 2, 100, 0xffffffff].map(release => ({
  release, expected: audio.hubTeacherSummonPitch(release),
}))
const releases = [[0, 2.67], [2.67, 2.68], [2.68, 11.15], [0, 50], [10, 0], [-5, 3]].map(
  ([previous, current]) => ({previous, current, expected: audio.hubTeacherReleasesBetween(previous, current)}),
)
writeFixture('audio-goldens.json', 'tools/website-oracle/audio-goldens.mjs',
  'Website/frontend/src/game/game-audio-native.ts', {
    entry, selection, attenuation, steps, teacher, releases,
    music: audio.GAME_SCENE_MUSIC,
    limits: audio.NATIVE_SOUND_MAXIMUM_VOICES,
  })
