import { loadKernel, writeFixture } from './oracle.mjs'
const kernel = await loadKernel('core-kernels/native-boneyard-lighting.ts')
const model = await loadKernel('core-kernels/native-boneyard-light-model.ts')
const cases=[]
for(const origin of [{x:0,y:0},{x:13.01,y:-27.03},{x:10000003.125,y:-10000003.875}]) {
  for(const radius of [.65,1,2.5999999046325684]) {
    const source={position:{x:origin.x+300.3,y:origin.y+200.2},radius,intensity:.713,castsDirectionalShadow:true}
    for(const offset of [[0,0],[74.99999,0],[75,0],[144.99999,0],[145,0],[0,80],[0,-80],[95,66],[-95,-66]]) {
      const position={x:source.position.x+offset[0]*radius,y:source.position.y+offset[1]*radius}
      const sources=[source,{position:{x:source.position.x-47.25,y:source.position.y+81.3},radius:.7,intensity:.32}]
      const query={cameraOrigin:origin,ambient:.3}
      cases.push({position,source,sources,query,
        factors:kernel.nativeBoneyardSourceLightFactors(position,source,query),
        combined:model.nativeBoneyardLightFactors(position,sources,query),
        scalar:model.nativeBoneyardLightScalar(position,sources,query),
        surface:model.nativeBoneyardSurfaceLightScalar(position,sources,query)})
    }
  }
}
for(const ambient of [0,.3,.75]) {
  const position={x:10,y:10},query={ambient},sources=[]
  cases.push({position,source:null,sources,query,combined:model.nativeBoneyardLightFactors(position,sources,query),
    scalar:model.nativeBoneyardLightScalar(position,sources,query),surface:model.nativeBoneyardSurfaceLightScalar(position,sources,query)})
}
const tints=[0,.1,.3,.5,.6,1,1.1,64/255,128/255,254/255].map(scalar=>({scalar,tint:model.nativeBoneyardLightTint(scalar)}))
writeFixture('current-light-query-goldens.json','tools/website-oracle/current-light-query-goldens.mjs',
  'Website@fa08ae249ff5563570bdb2a653e2ef9358ce7b83/core-kernels/native-boneyard-light{ing,-model}.ts',{cases,tints})
