import { createHash } from 'node:crypto'
import { loadKernel, writeFixture } from './oracle.mjs'

const { compileModAssets } = await loadKernel('modding/assets/mod-asset-compiler.ts')
const { compileWebLuaDefinition } = await loadKernel('modding/definition/web-lua-definition-compiler.ts')
const identity = { id: 'asset.test', name: 'Asset Test', version: '1.0.0' }
const location = { column: 0, file: 'scripts/main.lua', line: 1 }
const bytes = text => new TextEncoder().encode(text)
const digest = data => createHash('sha256').update(data).digest('hex')
const png = (width, height) => {
  // The source trust boundary checks this header, not pixel decompression.
  const data = new Uint8Array(24)
  data.set([0x89,0x50,0x4e,0x47,0x0d,0x0a,0x1a,0x0a])
  const view = new DataView(data.buffer)
  view.setUint32(8,13); data.set(bytes('IHDR'),12)
  view.setUint32(16,width); view.setUint32(20,height)
  return data
}
const specs = [
  ['sprite', 'full', { path:'art/full.png' }, png(17,23), 'image', 'image/png'],
  ['sprite', 'trim', { path:'art/trim.png', frame:{ x:2,y:3,width:7,height:9,logical_width:20,logical_height:30,center_x:-2.5,center_y:1.25,content_width:6.5,content_height:8.5 } }, png(17,23), 'image', 'image/png'],
  ['sprite', 'frames', { path:'art/frames.png', frames:[{width:8,height:8},{x:8,y:8,width:8,height:8}] }, png(16,16), 'image', 'image/png'],
  ['sheet', 'headings', { image:'art/headings.png', frame:{width:8,height:10}, headings:4, animations:{ idle:[1],walk:[1,3,2] } }, png(32,30), 'image', 'image/png'],
  ['sheet', 'simple', { image:'art/simple.png', frame:{width:8,height:10}, animations:{ once:[1,6] } }, png(24,20), 'image', 'image/png'],
  ['sound', 'wav', {path:'audio/sound.wav',volume:0.25}, bytes('RIFF0000WAVE'), 'audio','audio/wav'],
  ['sound', 'ogg', {path:'audio/sound.ogg'}, bytes('OggS'), 'audio','audio/ogg'],
  ['sound', 'mp3', {path:'audio/sound.mp3'}, Uint8Array.of(0xff,0xe0), 'audio','audio/mpeg'],
  ['music', 'music', {path:'audio/music.mp3',volume:0.6,loop:false}, bytes('ID3'), 'audio','audio/mpeg'],
  ['scene', 'scene', {path:'scenes/hall.json'}, bytes(JSON.stringify({schemaVersion:1,name:'Hall',rooms:['first','second']})), 'scene','application/json'],
  ['boneyard','yard',{path:'levels/yard.boneyard'},Uint8Array.of(1),'boneyard','application/vnd.solomon-dark.boneyard'],
]
function inputFor(spec) {
  const [assetKind,key,fields,data,kind,contentType] = spec
  const path = fields.path ?? fields.image
  const mod = compileWebLuaDefinition(identity,{ api:'1.0.0', assets:[{assetKind,key,fields,kind:'asset-definition',source:location}],content:[],reducers:[],rules:[] })
  return { assets:[{byteLength:data.length,contentType,kind,modId:identity.id,path,sha256:digest(data)}],
    mods:[structuredClone(mod)], sources:[{identity,files:{[path]:[...data]}}] }
}
function run(name,input) {
  const decoded = { ...input, sources:input.sources.map(source=>({...source,files:Object.fromEntries(Object.entries(source.files).map(([key,value])=>[key,Uint8Array.from(value)]))})) }
  try { return { name,input,output:compileModAssets(decoded).all(),error:null } }
  catch(error) { return { name,input,output:null,error:error.message } }
}
const cases = specs.map(spec=>run(spec[1],inputFor(spec)))
function change(name,spec,mutate) {
  const input = inputFor(spec)
  mutate(input)
  cases.push(run(name,input))
}
change('digest-changed',specs[0],input=>input.sources[0].files['art/full.png'][23]^=1)
change('length-changed',specs[0],input=>input.assets[0].byteLength+=1)
change('metadata-kind',specs[0],input=>input.assets[0].kind='audio')
change('missing-file',specs[0],input=>delete input.sources[0].files['art/full.png'])
change('duplicate-source',specs[0],input=>input.sources.push(input.sources[0]))
change('duplicate-metadata-case',specs[0],input=>input.assets.push({...input.assets[0],modId:'ASSET.TEST',path:'ART/FULL.PNG'}))
change('bad-path',specs[0],input=>input.mods[0].assets[0].fields.path='art/../outside.png')
change('bad-header',specs[0],input=>{ input.sources[0].files['art/full.png'][12]=0; input.assets[0].sha256=digest(Uint8Array.from(input.sources[0].files['art/full.png'])) })
change('sheet-remainder',specs[3],input=>input.mods[0].assets[0].fields.frame.width=7)
change('sheet-heading-count',specs[3],input=>input.mods[0].assets[0].fields.headings=3)
change('animation-range',specs[3],input=>input.mods[0].assets[0].fields.animations.walk=[4])
change('frame-outside',specs[1],input=>input.mods[0].assets[0].fields.frame.x=16)
change('frame-unknown-field',specs[1],input=>input.mods[0].assets[0].fields.frame.unknown=1)
change('bus-mismatch',specs[8],input=>input.mods[0].assets[0].fields.bus='effects')
change('audio-header',specs[5],input=>{ input.sources[0].files['audio/sound.wav'][8]=0; input.assets[0].sha256=digest(Uint8Array.from(input.sources[0].files['audio/sound.wav'])) })
change('scene-unknown-field',specs[9],input=>{ const data=bytes('{"schemaVersion":1,"name":"Hall","rooms":["first"],"extra":1}'); input.sources[0].files['scenes/hall.json']=[...data]; input.assets[0].sha256=digest(data); input.assets[0].byteLength=data.length })
writeFixture('mod-assets-goldens.json','mod-assets-goldens.mjs','modding/assets/mod-asset-compiler.ts',{cases})
