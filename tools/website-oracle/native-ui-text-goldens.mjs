// Exact authored fonts and all branches of the shared native UI text layout.
import { loadKernel, writeFixture } from './oracle.mjs'

const text = await loadKernel('native-ui/native-ui-text.ts')
const catalog = await loadKernel('native-ui/native-ui-catalog.ts')
const fonts = catalog.NATIVE_UI_FONT_NAMES.map(name => {
  const font = catalog.nativeUiFont(name)
  return {
    name,
    atlas: font.atlas,
    atlasDimensions: catalog.nativeUiAtlas(font.atlas).dimensions,
    metrics: font.metrics,
    spaceAdvance: font.spaceAdvance,
    glyphs: Object.entries(font.glyphs).map(([codePoint, glyph]) => ({
      codePoint: Number(codePoint), record: glyph.record, frame: glyph.frame,
      logicalSize: glyph.logicalSize, trimOrigin: glyph.trimOrigin, metrics: glyph.metrics,
    })).sort((left, right) => left.codePoint - right.codePoint),
    kerning: [...font.kerning].sort((left, right) => left[0] - right[0] || left[1] - right[1]),
    kerningWidths: font.kerning.map(([left, right]) => ({
      left, right,
      width: text.measureNativeUiText(String.fromCodePoint(left, right), name),
    })),
  }
})

function layoutRecord(layout) {
  return {
    font: layout.font, width: layout.width, height: layout.height,
    lines: layout.lines, unsupportedCodePoints: layout.unsupportedCodePoints,
    glyphs: layout.glyphs.map(glyph => ({
      record: glyph.record, codePoint: glyph.codePoint,
      centerX: glyph.centerX, centerY: glyph.centerY,
      scale: glyph.scale, alpha: glyph.alpha, tint: glyph.tint, italic: glyph.italic,
      ink: text.nativeUiGlyphInkBounds(glyph),
    })),
  }
}

const layouts = []
for (const font of fonts) {
  const alphabet = font.glyphs.map(glyph => String.fromCodePoint(glyph.codePoint)).join('')
  for (const scale of [.56, 1, 1.5]) {
    for (const align of ['left', 'center', 'right']) {
      const spec = { font: font.name, text: alphabet, x: 12.25, y: 37.5, align, scale, tint: 0xd9ba70, alpha: .625 }
      layouts.push({ spec, expected: layoutRecord(text.layoutNativeUiText(spec)) })
    }
  }
  for (const value of ['AV  AV', 'A\n\nV', '\nA', 'A\n', '', ' ☃☂', 'A☃V☂', 'A\tV\ufeffTo\u0085word']) {
    for (const placement of ['baseline', 'box']) {
      const spec = { font: font.name, text: value, x: 0, y: 10, align: 'left', maxWidth: 42, lineHeight: 25, placement }
      layouts.push({ spec, expected: layoutRecord(text.layoutNativeUiText(spec)) })
    }
  }
}
const runs = []
const wrappedRuns = []
for (const font of fonts) {
  for (const parts of [
    [{ text: 'A' }, { text: 'V', italic: true }],
    [{ text: 'A☃' }, { text: 'V☂', italic: true }],
    [{ text: '19' }, { text: ' /sec', scale: .7, advanceScale: 1, offsetX: 2, offsetY: -3, italic: true }],
    [{ text: 'AV  ' }, { text: 'AV\n\nA', italic: true, scale: .9 }],
  ]) {
    const spec = { font: font.name, runs: parts, tint: 0x90cf70, x: 10, y: 20 }
    runs.push({ spec, expected: layoutRecord(text.layoutNativeUiTextRuns(spec)) })
    for (const maxWidth of [40, 100, 400]) {
      wrappedRuns.push({ font: font.name, runs: parts, maxWidth,
        expected: text.wrapNativeUiTextRuns(parts, font.name, maxWidth) })
    }
  }
}
const msgBoxes = [
  { font: 'medium', text: 'ONE  TWO', maxWidth: 400 },
  { font: 'medium', text: 'ONE\nTWO', maxWidth: 400 },
  { font: 'menu', text: 'AV AV', maxWidth: 40 },
  { font: 'menu', text: 'AV   AV', maxWidth: 50 },
  { font: 'body', text: 'Longword-hyphenated    dialogue with  spaces and\r\nnewlines.', maxWidth: 45 },
  { font: 'medium', text: 'A surprisingly long thaumaturgical word', maxWidth: 90 },
].map(spec => ({ ...spec, expected: text.wrapNativeUiMsgBoxText(spec.text, spec.font, spec.maxWidth) }))
writeFixture('native-ui-text-goldens.json', 'tools/website-oracle/native-ui-text-goldens.mjs',
  'Website/frontend/src/game/native-ui/{native-ui-text,native-ui-catalog}.ts and assets/game/native-ui-assets.json',
  { fonts, layouts, runs, wrappedRuns, msgBoxes })
