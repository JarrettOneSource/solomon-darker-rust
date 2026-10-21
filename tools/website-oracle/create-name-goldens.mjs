import { loadKernel, writeFixture } from './oracle.mjs'
const names = await loadKernel('create-wizard-name.ts')
const values = new Set([
  ...names.STOCK_WIZARD_NAMES,
  ...'!,./0123456789:?ABCDEFGHIJKLMNOPQRSTUVWXYZ',
  '', ' ', 'Helvidius', 'SolonSolus', 'Solon-Solus', 'Account-Smoke_7', '___',
  'ſı', 'ß', '😀', 'A'.repeat(11), 'A'.repeat(12), 'A'.repeat(64), 'A'.repeat(65),
  'Offline Wizard', 'Peer Host', 'Peer Guest', 'Offline', 'PeerHost', 'PeerGuest',
])
const cases = [...values].map(value => {
  const validation = names.validateCreateWizardName(value)
  let layout = null
  if (validation.ok) {
    const native = names.layoutCreateWizardName(value)
    layout = { ...native, glyphs: native.glyphs.map(glyph => ({
      record: glyph.record, centerX: glyph.centerX, centerY: glyph.centerY,
    })) }
  }
  return { value, initial: names.initialCreateWizardName(value), validation,
    finalization: names.finalizeCreateWizardName(value), layout }
})
writeFixture('create-name-goldens.json', 'tools/website-oracle/create-name-goldens.mjs',
  'website-rust-current/frontend/src/game/create-wizard-name.ts', { cases })
