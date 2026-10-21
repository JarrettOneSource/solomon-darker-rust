// Actual Source Canvas2D measurements are fixture inputs. The Rust replay checks
// wrapping independently of the installed platform font's numeric measurements.
import { createRequire } from 'node:module'
import { resolve } from 'node:path'
import { websiteDir, writeFixture } from './oracle.mjs'

const frontend = resolve(websiteDir, 'frontend')
const require = createRequire(resolve(frontend, 'package.json'))
const { build } = require('esbuild')
const { chromium } = require('playwright-core')
const bundle = await build({
  stdin: { resolveDir: frontend, sourcefile: 'sack-name-oracle.ts', loader: 'ts', contents: `
    import { createBrowserSackName } from './src/game/renderer/hub-inventory/sack-name.ts';
    import { CanvasTextMetrics } from 'pixi.js';
    const context = CanvasTextMetrics._context;
    const measure = context.measureText.bind(context);
    const split = CanvasTextMetrics.graphemeSegmenter;
    const advances = Object.create(null);
    const graphemes = Object.create(null);
    context.measureText = value => {
      const result = measure(value);
      advances[value] = result.width;
      return result;
    };
    CanvasTextMetrics.graphemeSegmenter = value => {
      const result = split(value);
      graphemes[value] = result;
      return result;
    };
    window.recordSackNames = inputs => {
      const cases = inputs.map(input => {
        const sample = createBrowserSackName(input.value,0xd9ba70,input.maxWidth,input.wrap);
        const metrics = CanvasTextMetrics.measureText(input.value,sample.text.style);
        const source = {width:sample.text.width,height:sample.text.height,
          baselineOffset:sample.baselineOffset,lines:metrics.lines,lineWidths:metrics.lineWidths};
        sample.text.destroy();
        return {input,source};
      });
      return {advances,graphemes,cases};
    };
  ` },
  bundle: true, write: false, format: 'iife', platform: 'browser', target: 'es2022',
})
const inputs = []
for (const value of [
  'Runes 🧙', 'Épreuve 🧙', 'TheVeryLongSackName🧙L1', '宝箱 ✨💀',
  'Crystals 🪄 and 🧪 Reagents', 'A🧑‍💻é🧙🏽B', 'Rune-box—of–supplies',
  '  🧙  supplies  ', '🧙\r\nsupplies', 'A\n\n🧙\n',
  ...[9,32,8192,8193,8194,8195,8196,8197,8198,8200,8201,8202,8287,12288,160,8199]
    .map(code => 'Á' + String.fromCodePoint(code) + '🧙' + String.fromCodePoint(code) + 'Z'),
]) {
  for (const maxWidth of [20, 110, 300]) inputs.push({value,maxWidth,wrap:true})
  inputs.push({value,maxWidth:1000,wrap:false})
}
const browser = await chromium.launch({headless:true,
  executablePath:process.env.CHROME ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome'})
try {
  const page = await browser.newPage()
  await page.setContent('<html><body></body></html>')
  await page.addScriptTag({content:bundle.outputFiles[0].text})
  const fixture = await page.evaluate(inputs => window.recordSackNames(inputs),inputs)
  writeFixture('sack-name-goldens.json','tools/website-oracle/sack-name-goldens.mjs',
    'website-rust-current/frontend/src/game/renderer/hub-inventory/sack-name.ts (Pixi8.19 CanvasTextMetrics)',fixture)
  console.log(fixture.cases.length + ' Source Sack layouts')
} finally { await browser.close() }
