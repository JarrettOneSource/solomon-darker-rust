// Settled native profiles and source validation, without live Session reconstruction.
import { readFileSync, writeFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { execFileSync } from 'node:child_process'
import { loadKernel, websiteDir, root, writeFixture } from './oracle.mjs'
const profileKernel = await loadKernel('save/portable-game-profile.ts')
const bridge = await loadKernel('save/native-save-bridge.ts')
const codec = await loadKernel('save/native-save-codec.ts')
const progression = await loadKernel('core-kernels/player-progression.ts')
const templateText = readFileSync(resolve(websiteDir, 'frontend/public/game/native/portable-profile-template.json'), 'utf8')
const template = JSON.parse(templateText)
writeFileSync(resolve(root, 'web/assets/native/portable-profile-template.json'), templateText)
const base64 = value => Buffer.from(value).toString('base64')
const bytes = value => new Uint8Array(Buffer.from(value,'base64'))
const archiveJSON = value => ({ darkdata: base64(value.darkdata), gamestate: base64(value.gamestate),
  retainedFiles: value.retainedFiles.map(file=>({ path:file.path, bytes:base64(file.bytes) })), runName:value.runName })
const save = { darkdata: bytes(template.files.darkdata.base64), gamestate: bytes(template.files.gamestate.base64), retainedFiles: [], runName:'wizard_1' }
const factory = async (value, warnings=[]) => profileKernel.createPortableGameProfileFromNative(value.darkdata,value.gamestate,value.runName,value.retainedFiles,warnings)
const profile = await factory(save)
const baseDocument = profileKernel.encodePortableGameProfile(profile)
const creates = []
async function capture(name, value, warnings=[]) {
  try { creates.push({name, input:archiveJSON(value), warnings, encoded:profileKernel.encodePortableGameProfile(await factory(value,warnings))}) }
  catch(error) { creates.push({name, input:archiveJSON(value),warnings,error:error.message}) }
}
await capture('original stock profile',save)
await capture('opaque siblings and explicit warnings',{...save,retainedFiles:[{path:'solomondark/savegames/wizard_1/luthacus.sav',bytes:Uint8Array.of(1,2,255,4)}]},['Retained caller warning'])
const wizard = bridge.decodeNativeGamestateWizard(save.gamestate)
const patch = {...wizard,permanentRanks:wizard.rows.map(row=>row.permanentRank)}
const mixed = bridge.patchNativeGamestate(save.gamestate,{...patch,selectedPrimarySkillId:52,skillQuickbar:[52,null,null,null,null,null,null,null]})
await capture('native Spell Welding warning',{...save,gamestate:mixed})
const plane = structuredClone(codec.parseNativeSyncBuffer(save.gamestate))
const header = plane.root.children[0].payload
new DataView(header.buffer,header.byteOffset).setInt32(header.length-4,80,true)
const binding = plane.root.children[1].children[0].payload
const booleanCount = new DataView(binding.buffer,binding.byteOffset).getUint32(0,true)
new DataView(binding.buffer,binding.byteOffset).setInt32(4+booleanCount+4+12*4,80,true)
await capture('live Plane Orb override warning',{...save,gamestate:codec.encodeNativeSyncBuffer(plane)})
const effective = structuredClone(codec.parseNativeSyncBuffer(bridge.patchNativeGamestate(save.gamestate,{...patch,learnedOrder:[...patch.learnedOrder,40]})))
const rowOffset=4+(82-40)*12
new DataView(effective.root.children[0].children[0].payload.buffer).setUint16(rowOffset+2,2,true)
await capture('equipment-only learned row filtered',{...save,gamestate:codec.encodeNativeSyncBuffer(effective)})
const inactive = structuredClone(effective)
new DataView(inactive.root.children[0].children[0].payload.buffer).setUint16(rowOffset+2,0,true)
await capture('inactive native learned row rejects',{...save,gamestate:codec.encodeNativeSyncBuffer(inactive)})
for (const [name,offset,value] of [['experience admission mismatch',-10,0],['random Boast mismatch',-9,1]]) {
  const next=structuredClone(codec.parseNativeSyncBuffer(save.gamestate));const payload=next.root.children[0].children[0].payload
  payload[payload.length+offset]=value
  await capture(name,{...save,gamestate:codec.encodeNativeSyncBuffer(next)})
}
const owned = bridge.patchNativeGamestate(save.gamestate,{...patch,perkSelectors:[21,27,27,1],perkCapacity:9,hagathaOwnership:Array.from({length:50},(_,i)=>[1,8,21,27,49].includes(i))})
await capture('ordered Tonics and unavailable ownership retained',{...save,gamestate:owned})

const cases=[]
async function check(name,changes=[],document=null) {
  const next=structuredClone(profile)
  for(const change of changes) {
    let owner=next
    for(const key of change.path.slice(0,-1))owner=owner[key]
    const key=change.path.at(-1)
    if(change.remove)delete owner[key];else owner[key]=change.value
  }
  const input=document??JSON.stringify(next)
  try { const output=await profileKernel.parsePortableGameProfile(input);cases.push({name,changes,...(document===null?{}:{document}),encoded:profileKernel.encodePortableGameProfile(output)}) }
  catch(error) { cases.push({name,changes,...(document===null?{}:{document}),error:error.message}) }
}
const set=(path,value)=>({path:path.split('.').map(key=>/^\d+$/.test(key)?Number(key):key),value})
await check('base parse')
await check('fractional and ECMAScript number spelling',[set('wizard.experience',0.0000001),set('wizard.experienceBonus',-0.000001),set('wizard.currentHealth',25.125)])
await check('integral decimal syntax',[],baseDocument.replace('"version":1','"version":1.0').replace('"gold":500','"gold":500.0'))
await check('32 supplementary characters',[set('wizard.name','🧙'.repeat(32))])
await check('DEL character retained in wizard name',[set('wizard.name','Wizard\u007f')])
await check('advanced unlocks are independently retained',[set('wizard.advancedUnlocks',Array(8).fill(true))])
await check('empty text',[],'')
await check('malformed JSON',[],'{')
await check('root not object',[],'[]')
for(const [name,changes] of [
 ['extra root key',[set('unexpected',true)]],['missing root key',[{path:['version'],remove:true}]],
 ['unsupported version',[set('version',2)]],['wrong retail identity',[set('retailExecutableSha256','0'.repeat(64))]],
 ['wrong format',[set('format','other')]],['profile not object',[set('profile',[])]],
 ['extra profile field',[set('profile.extra',true)]],['materialized storage claim',[set('profile.nativeStorage.materializedInWeb',true)]],
 ['storage count',[set('profile.nativeStorage.childCount',1000001)]],['storage length',[set('profile.nativeStorage.payloadLength',67108865)]],
 ['Dowsing fee negative',[set('profile.dowsingFee',-1)]],['gold noninteger',[set('profile.gold',0.5)]],
 ['short firstMix',[set('profile.firstMixed',[])]],['nonboolean firstMix',[set('profile.firstMixed.0',1)]],
 ['short help flags',[set('profile.helpPending',[])]],['bad tutorial flag',[set('profile.tutorialPending',1)]],
 ['bundle duplicate',[set('profile.hagathaBundleSelectors',[1,1])]],
 ['Boast selected out of range',[set('profile.boast.selected',5)]],
 ['null Boast failure',[set('profile.boast.failed',true)]],
 ['random Boast failure',[set('profile.boast.selected',3),set('profile.boast.failed',true)]],
 ['terminal Boast conflict',[set('profile.boast.selected',0),set('profile.boast.failed',true),set('profile.boast.succeeded',true)]],
 ['empty wizard name',[set('wizard.name','')]],['long UTF16 name',[set('wizard.name','a'.repeat(65))]],
 ['long supplementary name',[set('wizard.name','🧙'.repeat(33))]],['control wizard name',[set('wizard.name','A\nB')]],
 ['warnings not strings',[set('warnings',[1])]],['short advanced flags',[set('wizard.advancedUnlocks',[])]],
 ['legacy discipline ID4 is retained',[set('wizard.disciplineRoot',4)]],
 ['root discipline lower bound',[set('wizard.disciplineRoot',-1)]],
 ['root discipline first accepted ID',[set('wizard.disciplineRoot',0)]],
 ['root discipline last accepted ID',[set('wizard.disciplineRoot',7)]],
 ['root discipline upper bound',[set('wizard.disciplineRoot',8)]],
 ['root element upper bound',[set('wizard.elementRoot',5)]],
 ['short concentration list',[set('wizard.concentrationSkillIds',[null])]],
 ['quickbar length',[set('wizard.skillQuickbar',[])]],['quickbar outside skill range',[set('wizard.skillQuickbar.0',80)]],
 ['duplicate learned order',[set('wizard.learnedOrder',[16,16])]],['inactive learned order',[set('wizard.learnedOrder',[40])]],
 ['short rank book',[set('wizard.permanentRanks',[])]],['negative rank',[set('wizard.permanentRanks.0',-1)]],
 ['large rank',[set('wizard.permanentRanks.0',65536)]],['bad concentration cursor',[set('wizard.nextConcentrationSlot',2)]],
 ['current HP exceeds maximum',[set('wizard.currentHealth',51)]],['current MP exceeds maximum',[set('wizard.currentMana',101)]],
 ['perk selector excluded8',[set('wizard.perkSelectors',[8])]],['perk duplicate ordinary',[set('wizard.perkSelectors',[1,1])]],
 ['third Tonic',[set('wizard.perkSelectors',[27,27,27])]],['perk capacity mismatch',[set('wizard.perkCapacity',4)]],
 ['ownership mismatch',[set('wizard.hagathaOwnership.1',true)]],
 ['source extra field',[set('nativeSource.extra',true)]],['source digest syntax',[set('nativeSource.darkdataSha256',profile.nativeSource.darkdataSha256.toUpperCase())]],
 ['source digest mismatch',[set('nativeSource.darkdataSha256','0'.repeat(64))]],['source malformed base64',[set('nativeSource.darkdataBase64','A')]],
 ['source empty payload',[set('nativeSource.darkdataBase64','')]],['source whitespace base64',[set('nativeSource.darkdataBase64',`${profile.nativeSource.darkdataBase64}\n`)]],
 ['source run name',[set('nativeSource.runName','../wizard')]],
 ['retained reserved settings',[set('nativeSource.retainedFiles',[{path:'solomondark/settings.txt',base64:'',sha256:await profileKernel.portableSha256(new Uint8Array())}])]],
 ['retained integrity mismatch',[set('nativeSource.retainedFiles',[{path:'solomondark/opaque',base64:'AA==',sha256:'0'.repeat(64)}])]],
]) await check(name,changes)
for(const [field,min,max] of [
 ['cheatDeathCharges',0,1000],['deferredSkillChoices',0,1000],['level',1,75],['meditationIdleDelay',-1,10000000],
 ['offerSeed',0,999999],['pendingSkillChoices',0,1000],['poisonImmunityTicks',0,10000000],['startingPrimary',8,79],['startingSecondary',8,79],
]) {
 await check(`${field} lower bound`,[set(`wizard.${field}`,min-1)])
 await check(`${field} upper bound`,[set(`wizard.${field}`,max+1)])
}
for(const [field,min,max] of [
 ['currentHealth',0,1000000],['currentMana',0,1000000],['experience',0,10000000],['experienceBonus',-1000,1000],
 ['manaCostReduction',-1000000,1000000],['maximumHealth',1,1000000],['maximumMana',1,1000000],
 ['nextThreshold',0,10000000],['offensiveDamageFlat',-1000000,1000000],['previousThreshold',0,10000000],['weldEffect',0,1000000],
]) {
 await check(`${field} finite lower bound`,[set(`wizard.${field}`,min-1)])
 await check(`${field} finite upper bound`,[set(`wizard.${field}`,max+1)])
}
const primary=Array.from({length:72},(_,i)=>i+8).find(id=>progression.nativeSkillCategory(id)===1)
const concentrations=Array.from({length:72},(_,i)=>i+8).filter(id=>progression.nativeSkillCategory(id)===3)
await check('nonprimary selection',[set('wizard.selectedPrimarySkillId',concentrations[0])])
await check('nonconcentration selection',[set('wizard.concentrationSkillIds',[primary,null])])
await check('duplicate concentrations',[set('wizard.concentrationSkillIds',[concentrations[0],concentrations[0]])])
await check('second concentration requires perk',[set('wizard.concentrationSkillIds',[null,concentrations[1]])])
await check('quickbar excludes concentration',[set('wizard.skillQuickbar.0',concentrations[0])])
const templateCases=[]
for(const [name,edit] of [
 ['embedded template',()=>{}],['template byte count',v=>v.files.darkdata.bytes++],['template run name',v=>v.expected.runName='../bad'],
 ['template base64 missing',v=>delete v.files.darkdata.base64],['template base64 malformed',v=>v.files.darkdata.base64='A'],
]) {
 const next=structuredClone(template);edit(next)
 try{const result=await bridge.loadNativeHubTemplate(async()=>new Response(JSON.stringify(next),{status:200}));templateCases.push({name,input:next,output:{darkdata:base64(result.darkdata),gamestate:base64(result.gamestate),retainedFiles:[],runName:result.runName}})}
 catch(error){templateCases.push({name,input:next,error:error.message})}
}
writeFixture('portable-game-profile-goldens.json', 'tools/website-oracle/portable-game-profile-goldens.mjs',
  'Website/frontend/src/game/save/portable-game-profile.ts and native-save-bridge.ts',
  { sourceRevision: execFileSync('git', ['-C', websiteDir, 'rev-parse', 'HEAD'], {encoding:'utf8'}).trim(),
    baseDocument, creates, cases, templateCases })
console.log(JSON.stringify({creates:creates.length,cases:cases.length,templateCases:templateCases.length}))
