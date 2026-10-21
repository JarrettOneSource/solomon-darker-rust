#!/usr/bin/env node
// Public action interpretation over the same real Source restores as the observer.
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { fixturesDir, loadKernel, writeFixture } from './oracle.mjs'

const game = await loadKernel('core-server/game-simulation.ts')
const saves = await loadKernel('save/game-save-document.ts')
const { MlBotPolicyObserver } = await loadKernel('core-server/ml-bot-policy/observer.ts')
const actions = await loadKernel('core-server/ml-bot-policy/actions.ts')
const observer = JSON.parse(readFileSync(resolve(fixturesDir, 'ml-bot-observer-goldens.json')))
const plain = value => JSON.parse(JSON.stringify(value, (_, item) => ArrayBuffer.isView(item) ? Array.from(item) : item))
const outcome = body => {
  try { return { value: plain(body()) } }
  catch (error) { return { error: error.message } }
}
const cases = []
const patches = [
  {}, { primary: 16 }, { primary: 40 }, { primary: 8, continuing: true },
  ...Array.from({ length: 10 }, (_, index) => ({ primary: 52, weld: 1000 + index, continuing: true })),
  { primary: 52, weld: null, continuing: true },
  { gated: 22 }, { gated: 23 }, { dead: true }, { noTargets: true },
  { targetId: 99999 }, { primary: 8, far: true }, { primary: 16, far: true },
  { primary: 8, affordable: false }, { primary: 8, ready: false },
  { rays: [0.12499999, 0.125, 0.12500001, 1, 0, 0.5, 0.25, 0.125] },
  { secondaryIds: [11,15,16,27,45,48,49,50] },
  { secondaryIds: [72,73,74,76,77,21,35,41] },
]
for (const [sourceIndex, original] of observer.cases.entries()) {
  const originalState = saves.restoreGameSaveDocument(JSON.stringify(original.document)).state
  const originalFrame = new MlBotPolicyObserver('local').observe(originalState, {
    activeInputs: original.activeInputs, controllers: original.controllers,
  })
  for (const [patchIndex, patch] of (sourceIndex === 2 ? patches : [{}]).entries()) {
    const state = structuredClone(originalState)
    const frame = structuredClone(originalFrame)
    const book = state.playerEntities.skillBooks[0]
    if (patch.primary !== undefined) book.primarySkillId = patch.primary
    if (patch.weld !== undefined) book.weldBuildId = patch.weld
    if (patch.continuing) frame.player.blockA[6] = 1
    if (patch.gated) frame.player.blockA[patch.gated] = 1
    if (patch.dead) state.playerEntities.progressions[0].lifeState = 'lethal-pending'
    if (patch.noTargets) { frame.enemyRows = []; frame.targetId = null }
    if (patch.targetId !== undefined) frame.targetId = patch.targetId
    if (patch.far) for (const row of frame.enemyRows) row.position = { x: 1900, y: 1900 }
    if (patch.affordable !== undefined) frame.player.primaryAffordable = patch.affordable
    if (patch.ready !== undefined) frame.player.blockA[7] = Number(patch.ready)
    if (patch.rays) frame.geometry.patchAndRays.set(patch.rays)
    if (patch.secondaryIds) frame.player.secondarySlots = patch.secondaryIds.map(skillId => ({
      affordable: true, occupied: true, ready: true, skillId,
    }))
    const mask = outcome(() => actions.createMlBotPolicyActionMaskPlan(state, 'local', frame))
    const decisions = []
    if (mask.value) {
      const plan = mask.value
      const firstTarget = plan.target.findIndex(value => value === 1)
      for (let movement = 0; movement < 9; movement += 1) {
        const action = { ability: 0, aim: 0, movement, target: firstTarget }
        decisions.push({ action, ...outcome(() => actions.resolveMlBotPolicyDecision(state, 'local', frame, action)) })
      }
      for (let target = 0; target < 9; target += 1) {
        for (let ability = 0; ability < 22; ability += 1) {
          const aims = plan.abilityByTarget[target][ability] === 1 ? [0, ...plan.aimByAbility[ability].slice(1).flatMap((value,index)=>value===1?[index+1]:[])] : [0]
          for (const aim of aims) {
            const action = { ability, aim, movement: 0, target }
            decisions.push({ action, ...outcome(() => actions.resolveMlBotPolicyDecision(state, 'local', frame, action)) })
          }
        }
      }
      for (const [head, count] of [['movement',9],['target',9],['ability',22],['aim',9]]) {
        const action = { ability: 0, aim: 0, movement: 0, target: firstTarget, [head]: count }
        decisions.push({ action, ...outcome(() => actions.resolveMlBotPolicyDecision(state, 'local', frame, action)) })
      }
    }
    cases.push({ name: `${original.name}:${patchIndex}`, sourceIndex, patch, mask, decisions })
  }
}
writeFixture('ml-bot-actions-goldens.json', 'tools/website-oracle/ml-bot-actions-goldens.mjs',
  'Website core-server/ml-bot-policy/actions.ts; real observer restores plus explicit action-boundary inputs', { cases })
console.log(JSON.stringify({ cases: cases.length, decisions: cases.reduce((sum,row)=>sum+row.decisions.length,0) }))
