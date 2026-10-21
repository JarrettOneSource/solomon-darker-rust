import { readFileSync, writeFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { pathToFileURL } from 'node:url'
import { fixturesDir, gameDir, loadKernel, websiteDir, writeFixture } from './oracle.mjs'

const chat = await loadKernel('hub-npc-dialogue.ts')
const native = await loadKernel('core-kernels/native-hub-npc.ts')
const presentation = await loadKernel('hub-inventory-presentation.ts')
const ui = await loadKernel('native-ui/core.ts')
const contract = await loadKernel('renderer/hub-inventory-render-contract.ts')
const catalog = JSON.parse(readFileSync(resolve(gameDir, 'native-hub-npc-catalog.json'), 'utf8'))
const records = Object.values({ ...catalog.dialogue, ...catalog.storyOffice.dialogue })
const families = []
const fresh = []
for (const id of presentation.HUB_INTERACTION_IDS) for (const storyOffice of [false, true]) {
  const npc = native.createNativeHubNpcState()
  const definition = presentation.hubInteractionDialogue(id, storyOffice)
  const consumed = new Set()
  const initial = chat.createHubNpcChatContent(id, npc, 1, null, storyOffice, null, consumed)
  fresh.push({ id, storyOffice, intro: initial, choices: chat.hubNpcChatChoices(id, storyOffice),
    questions: definition.questions.map(row => ({ key: row.key, content: chat.hubNpcQuestion(id, row.key, storyOffice) })),
    dismissal: chat.hubNpcDismissal(id, -7, storyOffice) })
  if (initial.kind === 'speech') chat.consumeHubNpcSpeech(consumed, id, initial.key, storyOffice)
  const afterIntro = [...consumed].sort()
  const questions = []
  for (const row of definition.questions) {
    const choices = chat.hubNpcChatChoices(id, storyOffice, consumed)
    const answer = chat.hubNpcQuestion(id, row.key, storyOffice, consumed)
    if (answer?.kind === 'speech') chat.consumeHubNpcSpeech(consumed, id, answer.key, storyOffice)
    questions.push({ key: row.key, choices, answer, consumed: [...consumed].sort(), again: chat.hubNpcQuestion(id, row.key, storyOffice, consumed) })
  }
  chat.consumeHubNpcSpeech(consumed, id, 'UNKNOWN_ROW', storyOffice)
  families.push({ id, storyOffice, initial, afterIntro, questions, consumed: [...consumed].sort(),
    finalChoices: chat.hubNpcChatChoices(id, storyOffice, consumed),
    reopened: chat.createHubNpcChatContent(id, npc, 1, null, storyOffice, null, consumed) })
}
const paragraphs = [...new Set(records.flatMap(row => row.lines))]
paragraphs.push('', '**empty*toggle', 'before *unmatched emphasis', 'a\nnew line', 'an-extremely-long-word-'.repeat(8))
const text = paragraphs.map(source => {
  const runs = ui.hubChatTextRuns(source)
  const lines = ui.wrapNativeUiTextRuns(runs, 'menu', 477)
  return { source, runs, lines: lines.map(line => line.flatMap(run => [...run.text].map(text => ({ text, italic: run.italic === true })))) }
})

// Execute the unchanged nested renderer function. Its closure inputs are actual
// source constants and a minimal speech model; expected clocks are not reimplemented.
const ts = (await import(pathToFileURL(resolve(websiteDir, 'frontend/node_modules/typescript/lib/typescript.js')).href)).default
const file = resolve(gameDir, 'renderer/hub-inventory-renderer.ts')
const ast = ts.createSourceFile(file, readFileSync(file, 'utf8'), ts.ScriptTarget.Latest, true, ts.ScriptKind.TS)
let renderChat
const visit = node => {
  if (ts.isFunctionDeclaration(node) && node.name?.text === 'renderChat') renderChat = node.getText(ast)
  ts.forEachChild(node, visit)
}
visit(ast)
if (!renderChat) throw new Error('Missing actual Source renderChat')
const javascript = ts.transpileModule(renderChat, { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.None } }).outputText
const sample = new Function('currentModel', 'chatRenderState', 'canvas', 'hubNativeUiElapsedTicks', 'HUB_NATIVE_UI_TIMING', 'HUB_CHAT_PANEL', 'nowMs', `${javascript}\nreturn renderChat(nowMs)`)
const frames = []
for (const acceleratedAt of [null, 1000, 1010.5, 2000]) for (const now of [0, 999, 1000, 1009, 1010, 2000, 5000, 50000]) for (const height of [0, 100.25, 500]) {
  const state = { content: { y: 0 }, contentHeight: height }
  const canvas = { dataset: {} }
  const complete = sample({ kind: 'dialogue', content: { kind: 'speech' }, phaseStartedAtMs: 1000, acceleratedAtMs: acceleratedAt }, state, canvas, contract.hubNativeUiElapsedTicks, contract.HUB_NATIVE_UI_TIMING, ui.HUB_CHAT_PANEL, now)
  frames.push({ acceleratedAt, now, height, complete, y: state.content.y })
}
writeFixture('hub-dialogue-goldens.json', 'hub-dialogue-goldens.mjs',
  'Current Website fa08ae249ff5563570bdb2a653e2ef9358ce7b83 hub-npc-dialogue.ts, authored oneShot rows, native emphasis/wrap and unchanged private renderChat',
  { records, families, text, frames })

// Only this changed family's old fresh-conversation section is replaced. Its
// independently retained geometry/markers/selectors remain their original evidence.
const legacyPath = resolve(fixturesDir, 'hub-interaction-goldens.json')
const legacy = JSON.parse(readFileSync(legacyPath, 'utf8'))
legacy.dialogues = fresh
legacy.dialogueSource = 'Current Website fa08ae249ff5563570bdb2a653e2ef9358ce7b83, hub-dialogue-goldens.mjs; other retained families unchanged'
writeFileSync(legacyPath, `${JSON.stringify(legacy)}\n`)
