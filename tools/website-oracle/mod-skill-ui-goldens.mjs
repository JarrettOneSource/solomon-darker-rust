// Render the unchanged Source components with React's server renderer. Only the
// TSX and CSS loading are adapted; no component logic or hook implementation is replaced.
import { readFileSync } from 'node:fs'
import { createRequire, registerHooks } from 'node:module'
import { pathToFileURL } from 'node:url'
import { resolve } from 'node:path'
import { gameDir, websiteDir, writeFixture } from './oracle.mjs'

const require = createRequire(resolve(websiteDir, 'frontend/package.json'))
const { transformSync } = require('esbuild')
const React = require('react')
const { renderToStaticMarkup } = require('react-dom/server')
registerHooks({
  load(url, context, nextLoad) {
    if (url.endsWith('.css')) return { format: 'module', source: '', shortCircuit: true }
    if (url.endsWith('.tsx')) {
      const source = readFileSync(new URL(url), 'utf8')
      const transformed = transformSync(source, { loader: 'tsx', jsx: 'automatic', format: 'esm' })
      return { format: 'module', source: transformed.code, shortCircuit: true }
    }
    return nextLoad(url, context)
  },
})
const { default: Book } = await import(pathToFileURL(resolve(gameDir, 'mod-ui/ModSkillBook.tsx')))
const { default: Picker } = await import(pathToFileURL(resolve(gameDir, 'mod-ui/ModSkillPicker.tsx')))
const skill = (content_id, extra = {}) => ({
  content_id, name: `Skill ${content_id}`, description: `Description ${content_id}`,
  mod_id: 'example.mod', icon_path: 'art/icon.png', maximum_rank: 5, ...extra,
})
const base = {
  skills: [skill('a'), skill('b', { parent_content_id: 'a' }), skill('c', { description: '' })],
  skill_ranks: [{ content_id: 'a', rank: 2 }, { content_id: 'b', rank: 1 }],
  spells: [skill('spell')], mod_quickbar: [],
  skill_offers: [{ player_id: 'self', sequence: 7, content_ids: ['c', 'a'] }],
}
const decode = text => text.replaceAll('<!-- -->', '').replaceAll('&amp;', '&').replaceAll('&lt;', '<')
  .replaceAll('&gt;', '>').replaceAll('&#x27;', "'").replaceAll('&quot;', '"')
const tag = (html, name) => decode(html.match(new RegExp(`<${name}[^>]*>(.*?)</${name}>`, 's'))?.[1] ?? '')
const cases = []
function record(name, runtime, player = 'self') {
  const session = { getModRuntime: () => runtime, onModRuntime: () => () => {}, playerId: player, modAssets: [] }
  const bookHtml = renderToStaticMarkup(React.createElement(Book, { session }))
  const pickerHtml = renderToStaticMarkup(React.createElement(Picker, { session, nativeOfferSequence: 19 }))
  const learned = [...bookHtml.matchAll(/<article([^>]*)>(.*?)<\/article>/gs)].map(([, attrs, html]) => {
    const [rank, maximum] = tag(html, 'span').slice(5).split(' / ').map(Number)
    return { name: tag(html, 'strong'), description: tag(html, 'small'), rank, maximum, subskill: attrs.includes('data-subskill="true"') }
  })
  const spells = [...bookHtml.matchAll(/<button([^>]*)>(.*?)<\/button>/gs)].map(([, attrs, html]) => {
    const label = tag(html, 'small')
    const bound = label.startsWith('Remove')
    const displayedSlot = Number(label.split(' ').at(-1))
    return { name: tag(html, 'span'), bound, slot: Number.isNaN(displayedSlot) ? null : displayedSlot - 1, disabled: attrs.includes('disabled=""') }
  })
  const offered = [...pickerHtml.matchAll(/<button[^>]*>(.*?)<\/button>/gs)].map(([, html]) => ({
    name: tag(html, 'span'), description: tag(html, 'small'),
  }))
  cases.push({ name, runtime, player, book: bookHtml ? { learned, spells } : null, picker: pickerHtml ? { offered } : null })
}
record('absent runtime', null)
record('empty runtime', {})
record('learned ranks and runtime order', base)
record('missing viewer offer', base, 'absent')
record('spells without learned skills', { ...base, skill_ranks: [] })
record('learned skills without spells', { ...base, spells: [] })
record('duplicate rank Map versus find', { ...base, skill_ranks: [{ content_id: 'a', rank: 1 }, { content_id: 'a', rank: 4 }], skills: [skill('a', { description: '' })] })
record('duplicate spell binding uses first', { ...base, mod_quickbar: [{ content_id: 'spell', slot: 5 }, { content_id: 'spell', slot: 2 }] })
record('all slots occupied', { ...base, mod_quickbar: Array.from({ length: 8 }, (_, slot) => ({ content_id: `other-${slot}`, slot })) })
record('occupied malformed slot counts as zero', { ...base, mod_quickbar: [{ content_id: 'other', slot: 'bad' }] })
record('negative bound slot retained', { ...base, mod_quickbar: [{ content_id: 'spell', slot: -3 }] })
record('no offered skill intersects', { ...base, skill_offers: [{ player_id: 'self', sequence: 7, content_ids: ['absent', 3, null] }] })
record('first local offer wins', { ...base, skill_offers: [{ player_id: 'other', sequence: 8, content_ids: ['b'] }, ...base.skill_offers, { player_id: 'self', sequence: 9, content_ids: ['b'] }] })
record('escaped authored text', { ...base, skills: [skill('a', { name: '<One & Two>', description: '"Apostrophe\'s"' })] })
for (const value of [-1, 0, .5, 1, 9_007_199_254_740_991, 9_007_199_254_740_992, null, '2', true]) {
  record(`offer sequence ${String(value)}`, { ...base, skill_offers: [{ player_id: 'self', sequence: value, content_ids: ['c'] }] })
  record(`rank and maximum ${String(value)}`, { ...base, skill_ranks: [{ content_id: 'a', rank: value }], skills: [skill('a', { maximum_rank: value, description: '' })] })
}
for (const value of [null, '', 'parent', 0, 1, false, true, [], {}]) {
  record(`parent truthiness ${JSON.stringify(value)}`, { ...base, skills: [skill('a', { parent_content_id: value })] })
}
record('invalid rows ignored', { skills: [null, [], 'bad', skill('a')], skill_ranks: [null, [], { content_id: 'a', rank: 1 }], spells: [5, false], skill_offers: [null, [], ...base.skill_offers] })
writeFixture('mod-skill-ui-goldens.json', 'tools/website-oracle/mod-skill-ui-goldens.mjs', 'Website mod-ui/ModSkillBook.tsx and ModSkillPicker.tsx rendered unchanged by React server renderer', { cases })
console.log(`${cases.length} Source component frames`)
