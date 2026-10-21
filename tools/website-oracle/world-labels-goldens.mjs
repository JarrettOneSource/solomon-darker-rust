// Actual world-label layout and receipt lifetimes; no renderer or authority substitute.
import { pathToFileURL } from 'node:url'
import { resolve } from 'node:path'
import { loadKernel, websiteDir, writeFixture } from './oracle.mjs'

const nameplates = await loadKernel('renderer/native-world-nameplate.ts')
const speech = await loadKernel('renderer/native-world-speech.ts')
const lifetime = await loadKernel('world-speech-presentation.ts')
const { buildCircle } = await import(pathToFileURL(resolve(websiteDir,
  'frontend/node_modules/pixi.js/lib/scene/graphics/shared/buildCommands/buildCircle.mjs')).href)
const glyphs = values => values.map(glyph => ({ record: glyph.record,
  centerX: glyph.centerX, centerY: glyph.centerY, scale: glyph.scale }))

const plates = ['', 'Morrow', 'AVATAR', 'lowercase ogre', 'ÝÅ☃', 'W'.repeat(36)].map(name => {
  const plan = nameplates.worldNameplateVisualLayout(name)
  return { name, ...plan, glyphs: glyphs(plan.glyphs) }
})
const messages = ['', 'ONE  TWO\nTHREE', 'W'.repeat(60), 'A\ufeffV\u0085To', '☃☂',
  'This is an ordinary party message with enough words to wrap across multiple lines.']
  .map(text => {
    const plan = speech.layoutNativeWorldSpeech(text)
    return { text, ...plan, lines: plan.lines.map(line => ({ ...line, glyphs: glyphs(line.glyphs) })) }
  })
const health = [[0,100],[-5,100],[25,100],[200,100],[1,0],[1,-1]].map(([current,maximum]) => ({
  current, maximum, expected: nameplates.nativeWorldNameplateHealthRatio(current,maximum),
}))
const projections = [[0,0],[.1,.1],[800,450],[801,450],[-.1,1]].map(([x,y]) => {
  const point={x,y},transform={position:{x:0,y:0},scale:2},viewport={width:1600,height:900}
  return {point,transform,viewport,expected:nameplates.projectNativeWorldPoint(point,transform,viewport)}
})
let speeches = []
const journey = []
const consume = (sequence, playerId, receivedAtMs, channel='global', record=true) => {
  const message = {sequence,channel,sender:{playerId,displayName:playerId,playerReference:playerId},text:`Message ${sequence}`}
  const next = lifetime.appendGameWorldSpeech(speeches,message,receivedAtMs)
  const step={message,receivedAtMs,accepted:next!==speeches}
  if(record) step.speeches=next
  journey.push(step)
  speeches=next
}
consume(1,'a',100)
consume(2,'b',200,'party')
consume(1,'a',300,'whisper')
consume(3,'a',400,'whisper')
for(let index=0;index<65;index++) consume(index+10,`speaker-${index}`,500+index,'global',index>=62)
const samples = [399,400,3400,4400,5399,5400].map(nowMs => ({nowMs,
  speech:journey[3].speeches.at(-1),expected:lifetime.sampleGameWorldSpeech(journey[3].speeches.at(-1),nowMs)}))
consume(100,'fresh',10000)
const shapes=[
  {type:'circle',x:0,y:0,radius:10},
  {type:'circle',x:-4.5,y:0,radius:1.5},
  {type:'roundedRectangle',x:-4.5,y:-5,width:3,height:10,radius:1},
  {type:'roundedRectangle',x:-42,y:-46,width:84,height:40,radius:4},
].map(shape=>{const points=[];buildCircle.build(shape,points);return{shape,points}})

writeFixture('world-labels-goldens.json','tools/website-oracle/world-labels-goldens.mjs',
  'Pinned Website renderer/native-world-{nameplate,speech}.ts, world-speech-presentation.ts and installed Pixi buildCircle',
  {plates,messages,health,projections,journey,samples,shapes})
