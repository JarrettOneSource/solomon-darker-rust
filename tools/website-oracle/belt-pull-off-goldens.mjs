import { loadKernel, writeFixture } from './oracle.mjs'

const source = await loadKernel('skill-book-model.ts')
const threshold = []
for (const origin of [[0, 0], [37, -82]]) {
  for (const delta of [[0, 0], [50, 0], [50.001, 0], [49.999, 0], [0, -50],
    [0, -50.001], [30, 40], [30.001, 40], [-30, -40], [35, 35], [36, 36]]) {
    const current = [origin[0] + delta[0], origin[1] + delta[1]]
    threshold.push({ origin, current, expected: source.nativeBeltPullOffStarted(
      { x: origin[0], y: origin[1] }, { x: current[0], y: current[1] },
    ) })
  }
}
const bursts = []
for (const step of [90, 120]) {
  for (const random of [[0], [0.5], [1], [-0.25, 1.25], [0.1, 0.7, 0.3, 0.9, 0.2]]) {
    let draws = 0
    const members = source.nativeBeltPullOffBurstMembers(step, () => random[draws++ % random.length])
    bursts.push({ step, random, draws, members })
  }
}
writeFixture('belt-pull-off-goldens.json', 'belt-pull-off-goldens.mjs',
  'Current Website fa08ae249ff5563570bdb2a653e2ef9358ce7b83 skill-book-model.ts public pull-off functions',
  { threshold, bursts })
