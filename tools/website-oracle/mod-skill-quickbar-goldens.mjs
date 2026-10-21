// Actual unchanged TSX render/click handlers and keyboard controller. The installed
// esbuild only strips TSX/types in memory; no source, asset or bundle is written.
import { createRequire } from 'node:module'
import { join } from 'node:path'
import { gameDir, loadKernel, websiteDir, writeFixture } from './oracle.mjs'

const require = createRequire(join(websiteDir, 'frontend/package.json'))
const React = require('react')
const jsx = require('react/jsx-runtime')
const { renderToStaticMarkup } = require('react-dom/server')
const { buildSync } = require('esbuild')
const { castModSkillQuickbarSpell, createModSkillQuickbarKeyboardController } =
  await loadKernel('mod-ui/mod-skill-quickbar-keyboard.ts')

let renderedButtons = []
function capture(renderer) {
  return (type, props, key) => {
    if (type === 'button') renderedButtons.push(props)
    return renderer(type, props, key)
  }
}
const source = buildSync({entryPoints: [join(gameDir, 'mod-ui/ModSkillQuickbar.tsx')],
  bundle: true, write: false, platform: 'node', format: 'cjs', jsx: 'automatic',
  external: ['react', 'react/jsx-runtime'], loader: {'.css': 'empty'}, logLevel: 'silent'})
const compiled = {exports: {}}
new Function('require', 'module', 'exports', source.outputFiles[0].text)(name =>
  name === 'react/jsx-runtime' ? {...jsx, jsx: capture(jsx.jsx), jsxs: capture(jsx.jsxs)} : require(name),
  compiled, compiled.exports)
const Quickbar = compiled.exports.default

const assets = [
  {byteLength: 1, contentType: 'image/png', kind: 'image', modId: 'quickbar.test', path: 'art/icon.png', sha256: 'a'.repeat(64)},
  {byteLength: 1, contentType: 'audio/ogg', kind: 'audio', modId: 'quickbar.test', path: 'audio/cue.ogg', sha256: 'b'.repeat(64)},
]
const spell = index => ({content_id: `spell-${index}`, name: `Spell ${index}`,
  mod_id: 'quickbar.test', icon_path: 'art/icon.png'})
const complete = {mod_quickbar: Array.from({length: 8}, (_, index) => ({slot: 7 - index, content_id: `spell-${7 - index}`})),
  spells: Array.from({length: 8}, (_, index) => spell(index))}
const player = {position: {x: 10.5, y: -20.25}, headingIndex: 7}

function component(name, runtime, player, modAssets = assets) {
  const previousWindow = globalThis.window
  globalThis.window = new EventTarget()
  renderedButtons = []
  const casts = []
  const session = {modAssets, playerId: 'viewer', getModRuntime: () => runtime,
    onModRuntime: () => () => {}, getSnapshot: () => ({players: player ? {viewer: player} : {}}),
    castModSpell: (contentId, target) => casts.push({contentId, target})}
  try {
    const html = renderToStaticMarkup(React.createElement(Quickbar, {session}))
    const slots = renderedButtons.map(button => {
      const image = button.children.find(child => child?.type === 'img')
      const label = button.children.find(child => child?.type === 'span').props.children
      const before = casts.length
      button.onClick()
      return {slot: label - 1, label, title: button.title, image: image?.props.src ?? null,
        cast: casts[before] ?? null}
    })
    return {name, runtime, player, assets: modAssets, visible: html.includes('<nav'), slots}
  } finally {
    if (previousWindow === undefined) delete globalThis.window
    else globalThis.window = previousWindow
  }
}

const components = [
  component('no-runtime', null, player),
  component('all-eight-accepted-bindings-sort-and-click', complete, player),
  component('last-spell-wins-and-missing-binding-is-omitted', {
    mod_quickbar: [null, [], {slot: 2, content_id: 'spell-1'}, {slot: 0, content_id: 'absent'}],
    spells: [false, spell(1), {...spell(1), name: 'Updated & precise', icon_path: 'absent.png'}],
  }, player),
  component('image-matching-is-exact-case-and-kind', {mod_quickbar: complete.mod_quickbar.slice(0, 3),
    spells: [{...spell(7), mod_id: 'QUICKBAR.TEST'}, {...spell(6), icon_path: 'Art/icon.png'},
      {...spell(5), icon_path: 'audio/cue.ogg'}]}, player),
  component('safe-slot-fallback', {mod_quickbar: [{slot: 1.5, content_id: 'spell-1'}], spells: [spell(1)]}, player),
  component('missing-player-keeps-ui-but-click-has-no-cast', complete, null),
]

const castTargets = []
for (let headingIndex = 0; headingIndex < 24; headingIndex++) {
  const actor = {...player, headingIndex}
  const casts = []
  castModSkillQuickbarSpell({playerId: 'viewer', getSnapshot: () => ({players: {viewer: actor}}),
    castModSpell: (contentId, target) => casts.push({contentId, target})}, spell(1))
  castTargets.push({player: actor, cast: casts[0]})
}

class KeyboardTarget {
  listeners = new Set()
  adds = 0
  removes = 0
  addEventListener(type, listener) { if (type !== 'keydown') throw new Error(type); this.adds++; this.listeners.add(listener) }
  removeEventListener(type, listener) { if (type !== 'keydown') throw new Error(type); this.removes++; this.listeners.delete(listener) }
}
const target = new KeyboardTarget()
const keyboard = createModSkillQuickbarKeyboardController(target)
const casts = []
let currentPlayer = player
const session = {playerId: 'viewer', getSnapshot: () => ({players: currentPlayer ? {viewer: currentPlayer} : {}}),
  castModSpell: (contentId, target) => casts.push({contentId, target})}
keyboard.update(session, [{slot: 0, spell: spell(1)}, {slot: 7, spell: spell(8)}])
const keys = [
  {key: '1', shiftKey: true}, {key: '8', shiftKey: true}, {key: '2', shiftKey: true},
  {key: '!', shiftKey: true}, {key: '*', shiftKey: true}, {key: 'Digit1', shiftKey: true},
  {key: '1', shiftKey: false}, {key: '1', shiftKey: true, ctrlKey: true},
  {key: '1', shiftKey: true, altKey: true}, {key: '1', shiftKey: true, metaKey: true},
  {key: '1', shiftKey: true, repeat: true}, {key: '1', shiftKey: true, editable: true},
  {key: '1', shiftKey: true, defaultPrevented: true}, {key: '1', shiftKey: true, missingPlayer: true},
  {key: '1.5', shiftKey: true}, {key: 'Infinity', shiftKey: true},
]
const events = keys.map(input => {
  currentPlayer = input.missingPlayer ? null : player
  const event = {...input, ctrlKey: input.ctrlKey ?? false, altKey: input.altKey ?? false,
    metaKey: input.metaKey ?? false, repeat: input.repeat ?? false,
    defaultPrevented: input.defaultPrevented ?? false, target: {tagName: input.editable ? 'INPUT' : 'DIV'},
    preventDefault() {this.defaultPrevented = true}}
  const before = casts.length
  for (const listener of target.listeners) listener(event)
  return {input, keyNumber: Number.isFinite(Number(input.key)) ? Number(input.key) : null,
    defaultPrevented: event.defaultPrevented, cast: casts[before] ?? null}
})
const lifecycle = []
const life = label => lifecycle.push({label, adds: target.adds, removes: target.removes, listeners: target.listeners.size})
life('first-live-bindings')
keyboard.update(session, [{slot: 3, spell: spell(3)}]); life('same-session-updated-binding')
keyboard.update({...session}, [{slot: 3, spell: spell(3)}]); life('different-session-reuses-handler')
keyboard.update(session, []); life('empty-detaches')
keyboard.update(session, []); life('empty-again-is-idempotent')
keyboard.update(session, [{slot: 0, spell: spell(1)}]); life('reappears')
keyboard.destroy(); life('destroy')
keyboard.destroy(); life('destroy-again')

writeFixture('mod-skill-quickbar-goldens.json', 'mod-skill-quickbar-goldens.mjs',
  'mod-ui/ModSkillQuickbar.tsx + mod-skill-quickbar-keyboard.ts + core-kernels/actor-heading.ts',
  {components, castTargets, events, lifecycle})
