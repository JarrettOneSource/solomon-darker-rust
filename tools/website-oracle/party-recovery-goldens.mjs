// Source signed owner continuations, including trust-boundary tampering and lineage bindings.
import { readFileSync } from 'node:fs'
import { createHmac } from 'node:crypto'
import { loadKernel, writeFixture, fixturesDir } from './oracle.mjs'
const recovery = await loadKernel('host/party-recovery-claim.ts')
const fixture = JSON.parse(readFileSync(`${fixturesDir}/save-document-goldens.json`, 'utf8'))
const sourceCase = fixture.cases.find(row => row.name === 'boneyard')
const document = structuredClone(sourceCase.document)
const playerId = document.continuation.summary.playerId
const config = sourceCase.restored.playerEntities.configs[0]
const combat = sourceCase.restored.playerEntities.progressions[0]
const secret = 'source-party-recovery-32-byte-secret'
const unsignedDocument = JSON.stringify(document)
const base = {
  contentManifestSha256: 'a'.repeat(64), globalScoreEligible: true, integrity: 'global-clean',
  leaderboardUserId: 42, partyMemberCount: 1, partyLeaderPlayerId: playerId,
  partyRoster: [{currentHealth: combat.currentHealth, displayName: config.displayName,
    element: config.element, lifeState: combat.lifeState, maximumHealth: combat.maximumHealth, playerId}],
  partyVisibility: 'public', playerId, recoveryId: 'r'.repeat(43),
  runId: document.continuation.simulation.run.runId, sessionKind: 'global-hub', targetRevision: null,
}
const creates = []
function create(name, change=()=>{}, key=secret, doc=unsignedDocument) {
  const claim = structuredClone(base)
  change(claim)
  const row = { name, claim, secret:key, document:doc }
  try { row.token = recovery.createPartyRecoveryClaim(key,claim,doc) }
  catch (error) { row.error = error.message }
  creates.push(row)
  return row
}
const original = create('global')
create('private', value => { value.sessionKind='private-college';value.globalScoreEligible=false;value.targetRevision='b'.repeat(40) })
create('standalone', value => {value.sessionKind='standalone';value.leaderboardUserId=null;value.partyVisibility='private'})
create('unicode-byte-secret', value => {value.partyRoster[0].displayName='🧙'.repeat(32)}, 'é'.repeat(16))
for (const [name, change] of [
  ['digest', value=>{value.contentManifestSha256='A'.repeat(64)}],
  ['account-zero', value=>{value.leaderboardUserId=0}],
  ['account-large', value=>{value.leaderboardUserId=2147483648}],
  ['members-zero', value=>{value.partyMemberCount=0}],
  ['members-large', value=>{value.partyMemberCount=17}],
  ['leader-empty', value=>{value.partyLeaderPlayerId=''}],
  ['roster-size', value=>{value.partyRoster=[]}],
  ['roster-duplicate', value=>{value.partyMemberCount=2;value.partyRoster.push({...value.partyRoster[0]})}],
  ['leader-absent', value=>{value.partyLeaderPlayerId='other'}],
  ['roster-name', value=>{value.partyRoster[0].displayName='😀'.repeat(33)}],
  ['roster-health', value=>{value.partyRoster[0].maximumHealth=0}],
  ['recovery-id', value=>{value.recoveryId='r'.repeat(42)}],
  ['run-mismatch', value=>{value.runId='different-run'}],
  ['revision', value=>{value.targetRevision='B'.repeat(40)}],
  ['global-local-only', value=>{value.integrity='local-only'}],
]) create(name,change)
create('short-secret', ()=>{}, 'short')
const signedDocument = (token, patch=()=>{}) => {
 const value=structuredClone(document);value.continuation.summary.partyRejoinToken=token;patch(value);return JSON.stringify(value)
}
const checks=[]
function check(name,token,doc=signedDocument(token),key=secret) {
 checks.push({name,token,document:doc,secret:key,
   decoded:recovery.decodePartyRecoveryClaim(key,token),verified:recovery.verifyPartyRecoveryClaim(key,token,doc),
   syntax:recovery.partyRecoveryClaimToken(token)})
}
check('ordinary',original.token)
check('unclaimed-document',original.token,unsignedDocument)
check('changed-owner',original.token,signedDocument(original.token,value=>{value.continuation.summary.playerId='other'}))
check('changed-gold',original.token,signedDocument(original.token,value=>{value.profile.economy.gold+=1}))
check('wrong-secret',original.token,signedDocument(original.token),'wrong-secret-also-longer-than-32bytes')
check('wrong-signature',original.token.slice(0,-2)+'ab')
check('old-prefix',original.token.replace(/^sdrpr2/,'sdrpr1'))
check('oversized',`sdrpr2.${'a'.repeat(8200)}.${'a'.repeat(43)}`)
const [prefix,payload,signature]=original.token.split('.')
const alphabet='ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_'
check('node-noncanonical-tail', `${prefix}.${payload}.${signature.slice(0,-1)}${alphabet[alphabet.indexOf(signature.at(-1))|1]}`)
const originalPayload=JSON.parse(Buffer.from(payload,'base64url').toString('utf8'))
for (const [name,change] of [
 ['version',value=>{value.version=1}],['unknown-key',value=>{value.extra=true}],
 ['missing-null-field',value=>{delete value.targetRevision}],
 ['payload-roster-extra',value=>{value.partyRoster[0].extra=true}],
 ['payload-element',value=>{value.partyRoster[0].element='ice'}],
 ['payload-life-state',value=>{value.partyRoster[0].lifeState='dead'}],
 ['payload-health-null',value=>{value.partyRoster[0].maximumHealth=null}],
]) {
 const modified=structuredClone(originalPayload);change(modified)
 const next=Buffer.from(JSON.stringify(modified)).toString('base64url')
 const token=`sdrpr2.${next}.${createHmac('sha256',secret).update(next).digest('base64url')}`
 check(name,token)
}
writeFixture('party-recovery-goldens.json','tools/website-oracle/party-recovery-goldens.mjs',
 'Website/frontend/src/game/host/party-recovery-claim.ts public create/decode/verify',{
  unsignedDocument,
  creates:creates.map(({document,...row})=>row),
  checks:checks.map(({document,...row})=>({...row,documentMode:
    row.name==='unclaimed-document'?'unsigned':row.name==='changed-owner'?'changed-owner':
    row.name==='changed-gold'?'changed-gold':'signed'})),
 })
