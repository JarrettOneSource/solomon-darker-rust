import { loadKernel, writeFixture } from './oracle.mjs'

const character = await loadKernel('core-kernels/player-character.ts')
const create = await loadKernel('core-kernels/native-create-loadout.ts')
const classes = await loadKernel('core-kernels/native-wizard-class.ts')
const progression = await loadKernel('core-kernels/player-progression.ts')
const rng = await loadKernel('core-kernels/native-rng.ts')

const identities = []
for (const element of character.WIZARD_ELEMENTS) {
  for (const discipline of character.WIZARD_DISCIPLINES) {
    const config = { displayName: 'Retained Name', element, discipline }
    identities.push({
      config,
      book: progression.createPlayerSkillBook(config),
      classTitle: classes.nativeWizardClassTitle(element, discipline),
    })
  }
}

const finalizations = []
function record(name, state, element, discipline, disabled = []) {
  const selection = { displayName: 'Retained Name', element, discipline }
  finalizations.push({
    name,
    state,
    selection,
    disabled,
    result: create.finalizeNativeCreateLoadout(state, selection, new Set(disabled)),
  })
}

// Seeded outcomes include the modulo-biased ordinary branch and the rejected
// same-element draw. Retain the whole random ring, rather than only its cursor.
for (const seed of [0, 4, 16, 28, 107]) {
  record(`seed-${seed}`, rng.createNativeRng(seed), null, null)
}
record('complete-selection', rng.createNativeRng(4), 'fire', 'body')
record('missing-element', rng.createNativeRng(4), null, 'body')
record('missing-discipline', rng.createNativeRng(4), 'fire', null)
record('filtered-primary', rng.createNativeRng(4), null, null, [16, 32])
record('empty-primary', rng.createNativeRng(4), null, 'mind', [8, 16, 24, 32, 40])
record('explicit-equal-roots', rng.createNativeRng(4), 'fire', 'fire')

// Source's rejection loop may consume several words before a different root.
const words = Array(55).fill(0)
for (const [index, word] of [1, 1, 1, 1, 4].entries()) words[index] = word << 6
record('repeated-same-root', { indexA: 0, indexB: 31, words }, null, null)

writeFixture('create-loadout-goldens.json', 'tools/website-oracle/create-loadout-goldens.mjs',
  'website-rust-current/frontend/src/game/core-kernels/native-create-loadout.ts', {
    sourceRevision: 'fa08ae249ff5563570bdb2a653e2ef9358ce7b83',
    identities,
    finalizations,
    entryFadeMs: create.NATIVE_CREATE_ENTRY_FADE_MS,
    randomizeFinalizeMs: create.NATIVE_CREATE_RANDOMIZE_FINALIZE_MS,
  })
