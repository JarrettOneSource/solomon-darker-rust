#!/usr/bin/env node
// Local Hall ranking and native row geometry from the unchanged pinned Website.
import {loadKernel, writeFixture} from './oracle.mjs'

const hall = await loadKernel('core-kernels/hall-of-fame.ts')
const view = await loadKernel('hall-of-fame-presentation.ts')
const classes = await loadKernel('core-kernels/native-wizard-class.ts')

const entries = [
  {runId:'first',awesomeness:100,wave:9,monstersKilled:40,elapsedTicks:12000},
  {runId:'second',awesomeness:200,wave:3,monstersKilled:40,elapsedTicks:200},
  {runId:'third',awesomeness:100,wave:9,monstersKilled:100,elapsedTicks:12000},
  {runId:'fourth',awesomeness:200,wave:3,monstersKilled:40,elapsedTicks:200},
]
const rankings = []
for (const board of hall.HALL_OF_FAME_BOARDS) {
  for (const limit of [0,1,3,100]) {
    rankings.push({board,limit,ids:hall.rankHallOfFameEntries(entries,board,limit).map(row=>row.runId)})
  }
}
const times = [0,99,100,5999,6000,359999,360000,12345678]
  .map(ticks=>({ticks,text:hall.formatHallOfFameTime(ticks)}))
const layouts = []
for (const top of [80,330,730]) for (const expanded of [false,true]) {
  for (const [rankWidth,awesomenessWidth] of [[24,120],[41,191]]) {
    const layout=view.hallRowLayout(top,expanded,rankWidth,awesomenessWidth)
    if (layout.expanded) layout.expanded.skillCells=layout.expanded.skillCells.map(cell=>({
      ...cell,badge:cell.badge(12),
    }))
    layouts.push({top,expanded,rankWidth,awesomenessWidth,layout,
      separators:view.hallSeparatorHalves(layout.separatorY)})
  }
}
const scroll = []
for (const expanded of [[],[false],[true],[false,true,false,true,false]]) {
  const rows=view.hallRowTops(expanded)
  for (const offset of [-20,0,200,600,1500]) {
    scroll.push({expanded,offset,...rows,extent:view.hallScrollExtent(rows.contentHeight),
      target:view.hallCurrentRowScrollTarget(rows.tops.at(-1)??0,rows.contentHeight),
      range:view.hallVisibleRowRange(rows.tops,expanded,offset),tile:view.hallTileOffset(offset)})
  }
}
const motion = [-1,0,1,30,89,90,120].map(tick=>({tick,
  ease:view.hallScrollEase(tick),fill:view.hallHighlightFillAlpha(tick),frame:view.hallCurrentFrameAlpha(tick)}))
const classTitles=[]
for(const element of ['air','earth','ether','fire','water']) {
  for(const discipline of ['arcane','body','mind']) {
    classTitles.push({element,discipline,title:classes.nativeWizardClassTitle(element,discipline)})
  }
}
writeFixture('hall-of-fame-goldens.json','tools/website-oracle/hall-of-fame-goldens.mjs',[
  'core-kernels/hall-of-fame.ts','hall-of-fame-presentation.ts','core-kernels/native-wizard-class.ts',
],{entries,rankings,times,layouts,scroll,motion,classTitles})
