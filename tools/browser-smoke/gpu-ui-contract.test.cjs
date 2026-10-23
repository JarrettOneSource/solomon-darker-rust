// Architectural acceptance: a GPU port must remove its legacy visual producers.
const test=require('node:test');
const assert=require('node:assert/strict');
const fs=require('node:fs');
const path=require('node:path');
const root=path.resolve(__dirname,'../..');
function rustFiles(directory) {
  return fs.readdirSync(directory,{withFileTypes:true}).flatMap(entry=>{
    const filename=path.join(directory,entry.name);
    return entry.isDirectory()?rustFiles(filename):filename.endsWith('.rs')?[filename]:[];
  });
}
const sources=rustFiles(path.join(root,'crates/web/src')).map(filename=>({
  filename:path.relative(root,filename),text:fs.readFileSync(filename,'utf8'),
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
      if(!/^"(?:canvas|input|textarea|audio)"\s*$/.test(match[1])) {
        forbidden.push(`${filename}: ${match[0]}`);
      }
    }
    if(/\.create_element_ns\s*\(|\.set_inner_html\s*\(/.test(text))forbidden.push(filename);
    if(/\bHtmlIFrameElement\b/.test(text))forbidden.push(`${filename}: visual iframe`);
  }
  assert.deepEqual(forbidden,[],'no legacy visual DOM factory, SVG or iframe survives');
});

test('the GPU renderer cannot rasterize HTML as a visual substitute',()=>{
  const forbidden=sources.filter(({text})=>/foreignObject|html2canvas|dom-to-image/.test(text))
    .map(({filename})=>filename);
  assert.deepEqual(forbidden,[]);
});
