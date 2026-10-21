import {loadKernel, writeFixture} from './oracle.mjs'
const source = await loadKernel('deployment-revision.ts')
const a='1'.repeat(40), b='2'.repeat(40), c='3'.repeat(40)
const responses=[]
for (const status of [200, 201, 404, 503]) {
  for (const body of [JSON.stringify({revision:a}), JSON.stringify({revision:' '+ 'A'.repeat(40)+' '}), '{}', 'null', '[]', '"text"', '{bad', JSON.stringify({revision:7}), JSON.stringify({revision:'main'}), JSON.stringify({revision:'a'.repeat(39)}), JSON.stringify({revision:'g'.repeat(40)})]) {
    responses.push({status,body,expected:await source.deploymentRevisionFromResponse(new Response(body,{status}))})
  }
}
const reloads=[]
for (const current of [a,b]) for (const live of [a,b,c]) for (const target of [null,a,b,c]) {
  reloads.push({current,live,target,expected:source.shouldReloadForDeployment(current,live,target)})
}
writeFixture('deployment-revision-goldens.json','deployment-revision-goldens.mjs','deployment-revision.ts (unmodified)',{responses,reloads})
