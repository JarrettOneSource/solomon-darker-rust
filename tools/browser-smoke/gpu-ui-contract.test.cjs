// Architectural acceptance: a GPU port must remove its legacy visual producers.
const test=require('node:test');
const assert=require('node:assert/strict');
const fs=require('node:fs');
const path=require('node:path');
const root=path.resolve(__dirname,'../..');
const fontProbePath='crates/web/src/gpu_ui/edit/font_run/browser.rs';
function rustFiles(directory) {
  return fs.readdirSync(directory,{withFileTypes:true}).flatMap(entry=>{
    const filename=path.join(directory,entry.name);
    return entry.isDirectory()?rustFiles(filename):filename.endsWith('.rs')?[filename]:[];
  });
}
const sources=rustFiles(path.join(root,'crates')).map(filename=>({
  filename:path.relative(root,filename).split(path.sep).join('/'),text:fs.readFileSync(filename,'utf8'),
}));

test('the game document installs only GPU canvases and bootstrap scripts',()=>{
  const document=fs.readFileSync(path.join(root,'web/index.html'),'utf8');
  const body=document.match(/<body[^>]*>([\s\S]*?)<\/body>/i)?.[1];
  assert.ok(body,'the game entry owns an explicit canvas document');
  const tags=[...body.matchAll(/<([a-z][a-z0-9-]*)\b/gi)].map(match=>match[1].toLowerCase());
  assert.ok(tags.includes('canvas'),'the game has a GPU canvas');
  assert.deepEqual(tags.filter(tag=>!['canvas','script'].includes(tag)),[],
    'visible bootstrap controls and SVG filters are retired');
  assert.equal(/<link\b[^>]*\brel=["']css["']/i.test(document),false,
    'the game entry has no visual CSS bundles');
});

test('Rust creates only platform adapter DOM elements',()=>{
  const forbidden=[];
  for(const {filename,text}of sources) {
    for(const match of text.matchAll(/\.create_element\s*\(\s*([^)]*)\)/g)) {
      const announcement=filename==='crates/graphics/src/announcements.rs'
        && /^"div"\s*$/.test(match[1]) && text.includes('data-gpu-announcer');
      // Unmounted anchors invoke the browser's native file-download capability.
      const platformTag=filename===fontProbePath?/^"canvas"\s*$/:/^"(?:canvas|input|textarea|audio|a)"\s*$/;
      if(!announcement&&!platformTag.test(match[1])) {
        forbidden.push(`${filename}: ${match[0]}`);
      }
    }
    for(const match of text.matchAll(/\.create_element_ns\s*\(([^)]*)\)/g)) {
      const fontProbe=filename===fontProbePath
        && /^\s*namespace,\s*"(?:svg|text)"\s*$/.test(match[1]);
      if(!fontProbe)forbidden.push(`${filename}: ${match[0]}`);
    }
    if(/\.set_inner_html\s*\(/.test(text))forbidden.push(filename);
    if(/\bHtmlIFrameElement\b/.test(text))forbidden.push(`${filename}: visual iframe`);
  }
  assert.deepEqual(forbidden,[],'no legacy visual DOM factory, SVG or iframe survives');
});

test('the browser font probe stays hidden, noninteractive and transient',()=>{
  const probe=sources.find(({filename})=>filename===fontProbePath);
  if(!probe)return;
  const {text}=probe;
  assert.deepEqual([...text.matchAll(/\.create_element_ns\s*\(\s*namespace,\s*"([^"]+)"\s*\)/g)]
    .map(match=>match[1]),['svg','text'],'the font probe has one SVG root and one text child');
  assert.deepEqual([...text.matchAll(/\.create_element\s*\(\s*"([^"]+)"\s*\)/g)]
    .map(match=>match[1]),['canvas'],'its only other element is the detached typography canvas');
  assert.match(text,/let namespace\s*=\s*Some\("http:\/\/www\.w3\.org\/2000\/svg"\)/);
  for(const [name,value]of [['data-gpu-font-metrics',''],['aria-hidden','true'],['focusable','false']]) {
    assert.ok(text.includes(`("${name}", "${value}")`),`font probe retains ${name}`);
  }
  assert.ok(text.includes('position:fixed;left:0;top:0;visibility:hidden;pointer-events:none;overflow:hidden'));
  assert.match(text,/let result\s*=\s*f\(probe\);\s*probe\.root\.remove\(\);\s*result/,
    'measurement success and failure both remove the native typography probe');
});

test('the GPU renderer cannot rasterize HTML as a visual substitute',()=>{
  const forbidden=sources.filter(({text})=>/foreignObject|html2canvas|dom-to-image/.test(text))
    .map(({filename})=>filename);
  assert.deepEqual(forbidden,[]);
});
