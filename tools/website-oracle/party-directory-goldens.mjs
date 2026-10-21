import { loadKernel, writeFixture } from './oracle.mjs'
const { normalizePartyCode, completePartyCode } = await loadKernel('party-join.ts')
const { directoryPartyAction, directoryPartyPresentation } = await loadKernel('party-directory.ts')
const values = new Set([
  '', 'ABCD', 'abcd-efgh', 'ABCDEFGH', '0000-1111', 'IOL0-1234',
  'prior-prefix-ABCD-EFGH', 'ſıß', 'ßßßßß', '😀ABCD😁EFGH', 'ABCD EFGH trailing',
  ...Array.from({length: 33}, (_, length) => 'ABCDefgh23456789'.repeat(3).slice(0, length)),
  ...'ABCDEFGHJKLMNPQRSTUVWXYZ23456789'.split('').map(char => char.repeat(12)),
])
const codes = [...values].map(value => ({value, normalized: normalizePartyCode(value), complete: completePartyCode(value)}))
const listings = []
for (const visibility of ['invite-only', 'public']) {
  for (const status of ['hub', 'playing']) {
    const party = {visibility, status, memberCount: 3, maxMembers: 8, boneyardName: status === 'playing' ? 'Random Level' : null}
    listings.push({...party, action: directoryPartyAction(party), presentation: directoryPartyPresentation(party)})
  }
}
writeFixture('party-directory-goldens.json', 'tools/website-oracle/party-directory-goldens.mjs',
  'Website/frontend/src/game/{party-join,party-directory}.ts', {codes, listings})
