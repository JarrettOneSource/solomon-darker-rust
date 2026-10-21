import { loadKernel, writeFixture } from './oracle.mjs'

const source = await loadKernel('match-loading.ts')
const stages = source.MATCH_LOADING_STAGE_DEFINITIONS.map(({ stage }) => stage)
const begins = []
for (const flow of ['hub', 'boneyard']) {
  for (const stage of stages) {
    begins.push({ flow, stage, expected: source.beginMatchLoading(flow, stage, 1000) })
  }
}
const sequences = []
for (const flow of ['hub', 'boneyard']) {
  for (const [name, start, requests] of [
    ['forward then regressions', stages[0], [...stages, ...stages.toReversed()]],
    ['equal-progress preserves initial label', 'preparing_host', ['receiving_host_checkpoint', 'preparing_host']],
  ]) {
    let loading = source.beginMatchLoading(flow, start, 1000)
    const steps = requests.map(stage => {
      loading = source.advanceMatchLoading(loading, stage)
      return { stage, expected: loading }
    })
    sequences.push({ flow, name, start, steps, completed: source.completeMatchLoading(loading) })
  }
}
const delay = [999, 1000, 1100, 1149, 1149.999, 1150, 1150.001, 2000].map(now => ({
  now,
  expected: source.shouldPresentMatchLoading(source.beginMatchLoading('hub', stages[0], 1000), now),
}))
writeFixture('match-loading-goldens.json', 'match-loading-goldens.mjs',
  'Current Website fa08ae249ff5563570bdb2a653e2ef9358ce7b83 match-loading.ts public lifecycle helpers',
  { delayMs: source.MATCH_LOADING_PRESENTATION_DELAY_MS, begins, sequences, delay })
