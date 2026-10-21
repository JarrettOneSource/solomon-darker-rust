import { loadKernel, writeFixture } from './oracle.mjs'

const terrain = await loadKernel('core-kernels/native-terrain-surface.ts')
const ground = await loadKernel('core-kernels/native-ground-auxiliary.ts')
const { NativeCompactGroundSurface } = await loadKernel('renderer/native-compact-ground-surface.ts')
const { nativeEnemySpriteRecord } = await loadKernel('renderer/native-enemy-assets.ts')
const strips = [
  { eid:'straight',pos:{x:700,y:800},uid:42001,style:0,points:[{x:100,y:200},{x:300,y:200}] },
  { eid:'curved',pos:{x:0,y:0},uid:-17,points:[{x:-25.125,y:80.5},{x:70.125,y:180.25},{x:240.5,y:70.75},{x:360.25,y:210.125}] },
  { eid:'short',pos:{x:300,y:300},uid:4294967297,points:[{x:100,y:200},{x:100.1,y:200.125},{x:100.25,y:200.2}] },
  { eid:'style-one',pos:{x:0,y:0},uid:23,style:1,points:[{x:100,y:200},{x:300,y:200}],profileSamples:[.25,-.5,1.25],sideSign:-1 },
  { eid:'empty',pos:{x:100,y:200},points:[] },
  { eid:'one-point',pos:{x:100,y:200},points:[{x:100,y:200}] },
]
const roads = [
  {eid:'crossing',typeId:3007,linkMask:0,points:[{x:200,y:100},{x:200,y:300}]},
  {eid:'angle',typeId:3004,linkMask:3,style:4,startWidthScale:4,endWidthScale:.1,points:[{x:90,y:100},{x:250,y:300},{x:900,y:900}]},
  {eid:'inside',typeId:3004,linkMask:0,points:[{x:125,y:200},{x:175,y:200}]},
  {eid:'touching',typeId:3004,linkMask:0,points:[{x:300,y:200},{x:500,y:200}]},
  {eid:'zero',typeId:3004,linkMask:0,points:[{x:200,y:200},{x:200,y:200}]},
]
const cases = strips.map(input => {
  const surface=terrain.buildNativeTerrainSurface(input)
  const bridges=terrain.buildNativeBridgeSurfaces([surface],roads)
  return {input,surface,bridges,queries:[{x:0,y:0},{x:150,y:200},{x:200,y:200},{x:150,y:246},
    ...surface.quads.flatMap(quad=>[quad[0],quad[3]])].map(point=>({point,
    quads:surface.quads.map(quad=>terrain.nativeSurfaceQuadContains(quad,point)),
    bridges:bridges.map(bridge=>terrain.nativeBridgeSurfaceContains(bridge,point))}))}
})
const base={name:'native surface',environmentMode:0,bounds:{x:0,y:0,w:800,h:600},
  spawn:{x:0,y:0,facingDeg:0},objects:[],fences:[],terrain:[strips[0]],roads:[roads[0]],solomonDig:null,
  sprites:[{eid:'compact',atlasEntry:25,pos:{x:200,y:200},s0:137,s1:.1,s2:0,flags:1}]}
const remains=[{id:1,spawnTick:0,state:{decal:{entry:140,position:{x:200,y:200},rotationDeg:91,scale:.1,alpha:.2}}}]
const queries=[]
for(const shape of ['river','bridge','style-one','all-selectors','strict-edge','outer-grid']) {
  const scene=structuredClone(base)
  if(shape==='river') {scene.roads=[];scene.sprites=[]}
  if(shape==='style-one') scene.terrain=[strips[3]]
  if(shape==='all-selectors') {scene.terrain=[];scene.roads=[];scene.sprites=[25,26,27,28,29].map((atlasEntry,index)=>({...base.sprites[0],eid:`s${index}`,atlasEntry,pos:{x:100+index*120,y:100}}))}
  if(shape==='strict-edge') {scene.terrain=[];scene.roads=[];scene.sprites[0].pos={x:100.000003,y:100}}
  if(shape==='outer-grid') {scene.terrain=[];scene.roads=[];scene.sprites=[-25,850].map(x=>({...base.sprites[0],eid:`x${x}`,pos:{x,y:100}}))}
  const query=new NativeCompactGroundSurface(scene)
  for(const point of [{x:150,y:200},{x:200,y:200},{x:150,y:246},{x:100,y:100},{x:220,y:100},
    {x:340,y:100},{x:460,y:100},{x:580,y:100},{x:141,y:94},{x:140.99,y:94},{x:-25,y:100},{x:850,y:100}]) {
    queries.push({shape,scene,point,withoutRemains:query.contains(point,[]),withRemains:query.contains(point,remains)})
  }
}
const glyphs=[]
for(const headingDegrees of [0,15,90,180,271.125,359.875]) {
  for(const [lifeState,deathTick,corpseConsumed] of [['alive',0,false],['lethal-pending',0,false],['dying',150,false],['dying',151,false],['spectating',150,false],['alive',0,true]]) {
    const player={position:{x:100.000003,y:201.125},headingDegrees,scale:1,lifeState,deathTick,corpseConsumed}
    glyphs.push({player,ordinary:ground.nativePlayerGroundGlyph(player,false),special:ground.nativePlayerGroundGlyph(player,true)})
  }
}
const sizes=[139,140,141,142,143].map(entry=>{const r=nativeEnemySpriteRecord('DeadHawg',entry);return{entry,width:r.width,height:r.height}})
writeFixture('terrain-surface-goldens.json','tools/website-oracle/terrain-surface-goldens.mjs',
  'Website@fa08ae249ff5563570bdb2a653e2ef9358ce7b83/core-kernels/native-{terrain-surface,ground-auxiliary}.ts; renderer/native-compact-ground-surface.ts',
  {cases,roads,queries,remains,sizes,glyphs})
