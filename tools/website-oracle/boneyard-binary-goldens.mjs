// Run the unchanged authored parser and projection on disk files and exact binary edits.
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { loadKernel, websiteDir, writeFixture } from './oracle.mjs'
const editor = await loadKernel('../editor/format/boneyard.ts')
const projection = await loadKernel('host/project-boneyard.ts')
const codec = await loadKernel('save/native-save-codec.ts')
const bytes64 = value => Buffer.from(value).toString('base64')
const cases = []
function capture(name, bytes) {
  try {
    const scene = projection.projectBoneyard(editor.parseBoneyard(bytes))
    cases.push({ name, bytes: bytes64(bytes), encoded: JSON.stringify(scene), geometrySha256: projection.boneyardGeometrySha256(scene) })
  } catch (error) {
    cases.push({ name, bytes: bytes64(bytes), error: error.message })
  }
}
for (const path of [
  'tests/fixtures/flat_multiplayer_test.boneyard',
  'backend/SeedAssets/survival.boneyard',
  'frontend/src/editor/format/blank-fixture.boneyard',
  'frontend/public/samples/story0.boneyard',
  'frontend/examples/web-lua/monument-crypt/levels/monument-crypt.boneyard',
]) capture(path, new Uint8Array(readFileSync(resolve(websiteDir,path))))
const blank = new Uint8Array(readFileSync(resolve(websiteDir,'frontend/src/editor/format/blank-fixture.boneyard')))
const doc = editor.newBoneyard('Authored 🧙 geometry',blank)
doc.timeline.records = []
doc.meta.header.environmentMode = 7
doc.meta.bounds.x = -25.25; doc.meta.bounds.y = 41.5; doc.meta.bounds.w=1234.5; doc.meta.bounds.h=987.25
doc.geometry.playerSpawn = { x:80.25,y:90.5 }; doc.geometry.playerSpawnFacingDeg=271.125
const position = {x:100.25,y:200.75}
doc.objects = [
  {eid:'authored-tree',typeId:2001,pos:position,variant:3,secondaryVariant:7,secondaryVisible:true,sortBias:1.25},
  {eid:'authored-monument',typeId:2009,pos:{x:140.5,y:250.25},variant:4,sortBias:-0.125},
  {eid:'authored-grave',typeId:2029,pos:{x:120.25,y:130.75},variant:2,overlayVariant:8,tint:{r:0.25,g:0.5,b:0.75,a:1}},
  {eid:'authored-building',typeId:2040,pos:{x:500.5,y:100.125},variant:6},
  {eid:'authored-goodie',typeId:2061,pos:{x:175.75,y:205.5},subtype:2,phase:1,active:true,timer:500,rewardSeed:123},
]
doc.roads = [0,1,2,3].map(link => ({eid:`road-${link}`,typeId:3004,uid:100+link,
  points:[{x:10+link,y:20.125},{x:30.5,y:80.25}], style:link,
  startWidthScale:0.25+link/8,endWidthScale:1.5+link/8,
  previousUid:link&1?12:0xffffffff,nextUid:link&2?15:0xffffffff,
}))
doc.fences = [
 {eid:'gate',typeId:3005,points:[{x:20,y:30},{x:130,y:60}],segmentCode:2,startPostVariant:7},
 {eid:'wall',typeId:3005,points:[{x:130,y:60},{x:200,y:80}],style:1,endPostVariant:9},
]
doc.terrain = [
 {eid:'terrain',typeId:3009,pos:{x:30,y:40},points:[{x:30,y:40},{x:50,y:20},{x:60,y:90}],style:6,profileSamples:[0.25,-0.5,1.25],sideSign:-1},
 {eid:'empty-terrain',typeId:3009,pos:{x:0,y:0},points:[],style:3,profileSamples:[]},
]
doc.sprites = [
 {eid:'ordinary',atlasEntry:0xffffffff,pos:{x:0.25,y:-20.5},s0:33.75,s1:0.625,s2:0.125,flags:255},
 {eid:'dirt-over-Solomon',atlasEntry:6,pos:{x:130.25,y:243.75},s0:0,s1:1,s2:1,flags:0},
 {eid:'rock-over-grave',atlasEntry:21,pos:{x:120.25,y:130.75},s0:90,s1:1,s2:1,flags:1},
 {eid:'dirt-over-grave-retained',atlasEntry:7,pos:{x:120.25,y:130.75},s0:0,s1:0.25,s2:1,flags:0},
 {eid:'rotated-distant',atlasEntry:22,pos:{x:900,y:900},s0:45,s1:1,s2:1,flags:1},
]
const authored = editor.serializeBoneyard(doc)
capture('all authored geometry, exact optional fields and Solomon clutter filtering',authored)
const scrubDoc = structuredClone(doc)
scrubDoc.objects = [15, 3, 16, 17, 18].map((variant, index) => ({
 eid:`stored-tree-${index}`,typeId:2001,pos:{x:100+index*40,y:200},variant,
 secondaryVariant:5,secondaryVisible:true,sortBias:99,scale:.5,rot:30,
}))
capture('stored Tree15..18 become fresh appended Scrubs without inherited Tree fields',editor.serializeBoneyard(scrubDoc))
const base = codec.parseNativeSyncBuffer(authored)
const layout = root => root.children[0].children[12].children[0]
const chunk = (payload=[],children=[]) => ({payload:Uint8Array.from(payload),children})
const u32 = value => {const out=new Uint8Array(4);new DataView(out.buffer).setUint32(0,value,true);return out}
const f32 = value => {const out=new Uint8Array(4);new DataView(out.buffer).setFloat32(0,value,true);return out}
const concat = values => new Uint8Array(Buffer.concat(values.map(value=>Buffer.from(value))))
const manager = (ids,children) => chunk(concat([u32(ids.length),...ids.map(u32)]),children)
function changed(name, edit) {
 const next=structuredClone(base);edit(next,layout(next.root));capture(name,codec.encodeNativeSyncBuffer(next))
}
changed('all known object kinds with opaque base shape',(buffer,sections)=>{for(let i=0;i<15;i+=3)sections.children[0].children[i].payload=Uint8Array.of(1)})
changed('all known object kinds with opaque subtype shape',(buffer,sections)=>{for(let i=2;i<15;i+=3)sections.children[0].children[i].payload=Uint8Array.of(1)})
changed('unknown objects are opaque single rows',(buffer,sections)=>{sections.children[0]=manager([9999],[chunk([1,2,3])])})
changed('opaque road fence terrain payloads',(buffer,sections)=>{for(const i of [5,6,12])for(const child of sections.children[i].children)child.payload=Uint8Array.of(1)})
changed('terrain trailing payload becomes opaque',(buffer,sections)=>{sections.children[12].children[0].payload=concat([sections.children[12].children[0].payload,Uint8Array.of(1)])})
changed('terrain counted arrays truncated become opaque',(buffer,sections)=>{new DataView(sections.children[12].children[0].payload.buffer).setUint32(8,0xffffffff,true)})
changed('opaque recipe payloads retain scene',(buffer,sections)=>{for(const i of [3,4,7,8,9,13])sections.children[i]=manager([6001,9999],[chunk([1]),chunk([])])})
changed('nonfinite f32 fields stringify to null',(buffer,sections)=>{
 const header=buffer.root.children[0].children[0].payload;const end=4+new DataView(header.buffer).getUint32(0,true)
 header.set(f32(NaN),end+519)
 sections.children[2].payload.set(f32(Infinity),0)
 sections.children[0].children[0].payload.set(f32(-Infinity),0)
 sections.children[11].payload.set(f32(NaN),4+12)
 sections.children[11].payload.set(f32(Infinity),4+16)
})
changed('header TextDecoder strips UTF8 BOM and replaces invalid bytes',(buffer)=>{
 const header=buffer.root.children[0].children[0].payload;const end=4+new DataView(header.buffer).getUint32(0,true)
 const name=Uint8Array.of(0xef,0xbb,0xbf,0x61,0xc0,0x80,0x62,0)
 buffer.root.children[0].children[0].payload=concat([u32(name.length),name,header.slice(end)])
})
changed('empty scene name retained',(buffer)=>{
 const header=buffer.root.children[0].children[0].payload;const end=4+new DataView(header.buffer).getUint32(0,true)
 buffer.root.children[0].children[0].payload=concat([u32(1),Uint8Array.of(0),header.slice(end)])
})
changed('nearest Solomon grave preserves first strict-distance tie',(buffer,sections)=>{
 const ids=manager([2029,2029],Array.from({length:6},(_,i)=>structuredClone(sections.children[0].children[6+i%3])))
 ids.children[3].payload.set(f32(40.25),0);ids.children[3].payload.set(f32(130.75),4)
 sections.children[0]=ids
})
for (const [name, edit] of [
 ['root payload',buffer=>buffer.root.payload=Uint8Array.of(1)],
 ['Arena child count',buffer=>buffer.root.children[0].children.pop()],
 ['Region payload',buffer=>buffer.root.children[0].children[12].payload=Uint8Array.of(1)],
 ['RegionLayout child count',(buffer,sections)=>sections.children.pop()],
 ['native String zero length',buffer=>buffer.root.children[0].children[0].payload.set(u32(0),0)],
 ['native String without terminator',buffer=>{const h=buffer.root.children[0].children[0].payload;h[4+new DataView(h.buffer).getUint32(0,true)-1]=1}],
 ['short Arena header',buffer=>buffer.root.children[0].children[0].payload=concat([u32(2),Uint8Array.of(65,0)])],
 ['truncated object manager',(buffer,sections)=>sections.children[0].payload=Uint8Array.of(1)],
 ['invalid object manager count',(buffer,sections)=>sections.children[0].payload.set(u32(500),0)],
 ['missing placeable chunk',(buffer,sections)=>sections.children[0].children.pop()],
 ['unclaimed object chunk',(buffer,sections)=>sections.children[0].children.push(chunk())],
 ['Road wrong ID',(buffer,sections)=>sections.children[5].payload.set(u32(9999),4)],
 ['Fence membership mismatch',(buffer,sections)=>sections.children[6].children.pop()],
 ['Terrain wrong ID',(buffer,sections)=>sections.children[12].payload.set(u32(3004),4)],
 ['sprite list truncated',(buffer,sections)=>sections.children[11].payload=new Uint8Array()],
 ['sprite count mismatch',(buffer,sections)=>sections.children[11].payload.set(u32(0xffffffff),0)],
 ['no player spawn',(buffer,sections)=>sections.children[2].payload=Uint8Array.of(0)],
 ...[3,4,7,8,9,13].map(i=>[`recipe manager ${i} mismatch`,(buffer,sections)=>sections.children[i]=manager([6001],[])]),
]) changed(name,edit)
capture('empty legacy bytes',new Uint8Array())
capture('truncated u32',Uint8Array.of(0,0,0))
capture('truncated payload',concat([u32(500),Uint8Array.of(1,2)]))
capture('trailing bytes',concat([authored,Uint8Array.of(0)]))
function encodeChunk(node) {return concat([u32(node.payload.length),node.payload,u32(node.children.length),...node.children.map(encodeChunk)])}
function encodeBuffer(root,named=[]) {return concat([encodeChunk(root),u32(named.length),...named.map(value=>concat([u32(value.name.length),value.name,encodeBuffer(value.root,value.named??[])]))])}
capture('duplicate nonUTF8 named buffers are allowed',encodeBuffer(base.root,[{name:Uint8Array.of(0xff,0),root:chunk()},{name:Uint8Array.of(0xff,0),root:chunk()}]))
for(const name of [new Uint8Array(),Uint8Array.of(65),Uint8Array.of(65,0,66,0)])capture(`invalid named bytes ${bytes64(name)}`,encodeBuffer(base.root,[{name,root:chunk()}]))
let deep=chunk();for(let i=0;i<514;i++)deep=chunk([], [deep]);capture('chunk nesting limit',encodeBuffer(deep))
capture('chunk count limit',concat([u32(0),u32(1000000)]))
capture('named count limit',concat([encodeChunk(base.root),u32(65537)]))
writeFixture('boneyard-binary-goldens.json', 'tools/website-oracle/boneyard-binary-goldens.mjs',
  'Website@fa08ae249ff5563570bdb2a653e2ef9358ce7b83/frontend/src/editor/format/boneyard.ts and game/host/project-boneyard.ts', {cases})
console.log(JSON.stringify({cases:cases.length,accepted:cases.filter(value=>value.encoded).length,rejected:cases.filter(value=>value.error).length}))
