// The canonical Rust shader is compiled unchanged. A native Canvas filter supplies
// the typography-independent Source CSS colour reference; no game UI is rendered here.
const fs = require('node:fs');
const path = require('node:path');
const assert = require('node:assert/strict');
const {chromium} = require(path.resolve(process.env.PW));
const sourcePath = process.env.GPU_SHADER_SOURCE || path.resolve(__dirname, '../../crates/graphics/src/lib.rs');
const source = fs.readFileSync(sourcePath, 'utf8');
const shader = name => {
  const found = source.match(new RegExp(`const ${name}: &str = r"([\\s\\S]*?)";`));
  assert.ok(found, `canonical ${name} is present`);
  return found[1];
};
const cases = [
  {name:'bright clipped translucent artwork', pixel:[220,160,60,128], brightness:1.4, saturation:1},
  {name:'disabled settings desaturation then brightness', pixel:[200,100,50,192], brightness:0.62, saturation:0.35},
  {name:'CSS luminance rather than Arena channel average', pixel:[255,0,0,255], brightness:1, saturation:0},
  {name:'black retains texture alpha', pixel:[12,230,80,64], brightness:0, saturation:1},
  {name:'identity preserves semi-transparent RGB', pixel:[40,80,120,128], brightness:1, saturation:1},
];
(async () => {
  const browser = await chromium.launch({headless:true,
    executablePath:process.env.CHROME || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    args:['--use-angle=swiftshader','--enable-unsafe-swiftshader','--ignore-gpu-blocklist']});
  try {
    const page = await browser.newPage();
    const rows = await page.evaluate(({vertex, fragment, cases}) => {
      const canvas = document.createElement('canvas');
      canvas.width = canvas.height = 1;
      const gl = canvas.getContext('webgl2', {premultipliedAlpha:false, preserveDrawingBuffer:true});
      if (!gl) throw Error('WebGL2 is unavailable');
      const compile = (type, source) => {
        const shader = gl.createShader(type);
        gl.shaderSource(shader, source); gl.compileShader(shader);
        if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) throw Error(gl.getShaderInfoLog(shader));
        return shader;
      };
      const program = gl.createProgram();
      gl.attachShader(program, compile(gl.VERTEX_SHADER, vertex));
      gl.attachShader(program, compile(gl.FRAGMENT_SHADER, fragment));
      gl.linkProgram(program);
      if (!gl.getProgramParameter(program, gl.LINK_STATUS)) throw Error(gl.getProgramInfoLog(program));
      gl.useProgram(program);
      const uniform = name => gl.getUniformLocation(program, name);
      gl.uniform2f(uniform('u_logical_size'), 1, 1);
      gl.uniform2f(uniform('u_pixel_center_offset'), 0, 0);
      gl.uniform1i(uniform('u_texture'), 0);
      gl.uniform1f(uniform('u_saturation'), 1);
      gl.uniform1f(uniform('u_alpha_mask'), 0);
      gl.uniform1f(uniform('u_white_alpha_filter'), 0);
      gl.uniform1f(uniform('u_premultiply_rgb'), 1);
      gl.uniform2f(uniform('u_shadow_sigma'), -1, -1);
      const vertices = new Float32Array([
        0,0, 0.5,0.5, 1,1,1,1, 1,0, 0.5,0.5, 1,1,1,1, 0,1, 0.5,0.5, 1,1,1,1,
        1,0, 0.5,0.5, 1,1,1,1, 1,1, 0.5,0.5, 1,1,1,1, 0,1, 0.5,0.5, 1,1,1,1,
      ]);
      gl.bindBuffer(gl.ARRAY_BUFFER, gl.createBuffer());
      gl.bufferData(gl.ARRAY_BUFFER, vertices, gl.STATIC_DRAW);
      for (const [name, size, offset] of [['a_position',2,0],['a_uv',2,8],['a_color',4,16]]) {
        const location = gl.getAttribLocation(program, name);
        gl.enableVertexAttribArray(location);
        gl.vertexAttribPointer(location, size, gl.FLOAT, false, 32, offset);
      }
      gl.bindTexture(gl.TEXTURE_2D, gl.createTexture());
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);
      gl.viewport(0, 0, 1, 1);
      const paint = (pixel, brightness, saturation) => {
        gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, 1, 1, 0, gl.RGBA, gl.UNSIGNED_BYTE, new Uint8Array(pixel));
        gl.uniform2f(uniform('u_color_filter'), brightness, saturation);
        gl.drawArrays(gl.TRIANGLES, 0, 6);
        const result = new Uint8Array(4);
        gl.readPixels(0, 0, 1, 1, gl.RGBA, gl.UNSIGNED_BYTE, result);
        return [...result];
      };
      const input = document.createElement('canvas'); input.width = input.height = 1;
      const output = document.createElement('canvas'); output.width = output.height = 1;
      const inputContext = input.getContext('2d');
      const outputContext = output.getContext('2d');
      const rows = cases.map(test => {
        inputContext.putImageData(new ImageData(new Uint8ClampedArray(test.pixel), 1, 1), 0, 0);
        outputContext.clearRect(0, 0, 1, 1);
        outputContext.filter = `saturate(${test.saturation}) brightness(${test.brightness})`;
        outputContext.drawImage(input, 0, 0);
        const rgba = [...outputContext.getImageData(0, 0, 1, 1).data];
        const expected = rgba.map((value, index) => index === 3 ? value : Math.round(value * rgba[3] / 255));
        const actual = paint(test.pixel, test.brightness, test.saturation);
        return {...test, actual, expected, delta:Math.max(...actual.map((value,index) => Math.abs(value-expected[index])))};
      });
      // Disable the scoped filter again; the world shader retains its original colour path.
      const restored = paint([40,80,120,128], -1, 1);
      rows.push({name:'filter restoration retains the world path', actual:restored,
        expected:[20,40,60,128], delta:Math.max(...restored.map((value,index)=>Math.abs(value-[20,40,60,128][index])))});
      gl.uniform1f(uniform('u_texture_premultiplied'),1);
      const premultiplied=paint([64,32,16,128],-1,1);
      rows.push({name:'GPU group textures preserve premultiplied RGB once',actual:premultiplied,
        expected:[64,32,16,128],delta:Math.max(...premultiplied.map((value,index)=>Math.abs(value-[64,32,16,128][index])))});
      gl.uniform1f(uniform('u_texture_premultiplied'),0);
      // An opaque neighboring atlas record deliberately surrounds the source crop.
      // Native Canvas filters only the extracted sprite and composites group opacity last.
      const atlas = new Uint8Array(12*12*4);
      const sprite = new Uint8ClampedArray(8*8*4);
      for(let y=0;y<12;y++)for(let x=0;x<12;x++) {
        atlas.set([255,0,255,255],(y*12+x)*4);
        if(x>=2&&x<10&&y>=2&&y<10) {
          const sx=x-2,sy=y-2;
          const alpha=sx>=2&&sx<=4&&sy>=1&&sy<=6?255
            :sx>=1&&sx<=6&&sy>=1&&sy<=6?128:0;
          const pixel=[200,160,50,alpha];
          atlas.set(pixel,(y*12+x)*4);sprite.set(pixel,(sy*8+sx)*4);
        }
      }
      const spriteCanvas=document.createElement('canvas');spriteCanvas.width=spriteCanvas.height=8;
      spriteCanvas.getContext('2d').putImageData(new ImageData(sprite,8,8),0,0);
      const shadowCases=[
        // Filter Effects permits the native three-box Gaussian approximation at
        // roughly 3%; nine bytes includes that variance and RGBA8 rounding.
        {name:'idle source shadow and outer opacity',sigma:1,offset:[0,1],color:[0,0,0,1],opacity:0.78,brightness:1,tolerance:9},
        {name:'hover brightness then colored source shadow',sigma:3,offset:[0,0],color:[255,240,183,0.55],opacity:1,brightness:1.25,tolerance:9},
        {name:'source and shadow compose before group opacity',sigma:0,offset:[0,0],color:[0,0,0,0.55],opacity:0.78,brightness:1,tolerance:1},
        {name:'mirrored crop preserves screen-space shadow offset',sigma:1,offset:[1,1],color:[0,0,0,1],opacity:1,brightness:1,mirror:true,tolerance:9},
        {name:'independent text shadow excludes the foreground source',sigma:2,offset:[0,1],color:[0,0,0,1],opacity:1,brightness:1,only:true,tolerance:9},
      ];
      canvas.width=96;canvas.height=64;gl.viewport(0,0,96,64);
      gl.uniform2f(uniform('u_logical_size'),96,64);
      gl.uniform4f(uniform('u_shadow_bounds'),2/12,2/12,10/12,10/12);
      gl.texImage2D(gl.TEXTURE_2D,0,gl.RGBA,12,12,0,gl.RGBA,gl.UNSIGNED_BYTE,atlas);
      gl.texParameteri(gl.TEXTURE_2D,gl.TEXTURE_MIN_FILTER,gl.LINEAR);
      gl.texParameteri(gl.TEXTURE_2D,gl.TEXTURE_MAG_FILTER,gl.LINEAR);
      for(const test of shadowCases) {
        const effect=document.createElement('canvas');effect.width=96;effect.height=64;
        const ctx=effect.getContext('2d');
        ctx.filter=`brightness(${test.brightness}) drop-shadow(${test.offset[0]}px ${test.offset[1]}px ${test.sigma}px rgba(${test.color.join(',')}))`;
        let inputSprite=spriteCanvas;
        if(test.mirror) {
          inputSprite=document.createElement('canvas');inputSprite.width=inputSprite.height=8;
          const mirrorContext=inputSprite.getContext('2d');mirrorContext.translate(8,0);mirrorContext.scale(-1,1);
          mirrorContext.drawImage(spriteCanvas,0,0);
        }
        if(test.only) {
          const alphaMask=document.createElement('canvas');alphaMask.width=alphaMask.height=8;
          const pixels=new Uint8ClampedArray(sprite);
          for(let index=0;index<pixels.length;index+=4) {
            pixels.set(test.color.slice(0,3),index);pixels[index+3]=Math.round(pixels[index+3]*test.color[3]);
          }
          alphaMask.getContext('2d').putImageData(new ImageData(pixels,8,8),0,0);
          ctx.filter=`blur(${test.sigma}px)`;ctx.drawImage(alphaMask,32+test.offset[0],24+test.offset[1]);
        } else ctx.drawImage(inputSprite,32,24);
        const result=document.createElement('canvas');result.width=96;result.height=64;
        const resultCtx=result.getContext('2d');resultCtx.globalAlpha=test.opacity;resultCtx.drawImage(effect,0,0);
        const reference=resultCtx.getImageData(0,0,96,64).data;
        const padding=test.sigma*3;
        const left=32+Math.min(0,test.offset[0]-padding),top=24+Math.min(0,test.offset[1]-padding);
        const right=40+Math.max(0,test.offset[0]+padding),bottom=32+Math.max(0,test.offset[1]+padding);
        const positions=[[left,top],[right,top],[right,bottom],[left,bottom]];
        const uv=positions.map(([x,y])=>[(test.mirror?10-x+32:2+x-32)/12,(2+y-24)/12]);
        const data=[];
        for(const index of [0,1,2,0,2,3])data.push(...positions[index],...uv[index],1,1,1,test.opacity);
        gl.bufferData(gl.ARRAY_BUFFER,new Float32Array(data),gl.STATIC_DRAW);
        gl.uniform2f(uniform('u_color_filter'),test.brightness,1);
        gl.uniform2f(uniform('u_shadow_sigma'),test.sigma/12,test.sigma/12);
        gl.uniform1f(uniform('u_shadow_only'),test.only?1:0);
        gl.uniform2f(uniform('u_shadow_offset'),(test.mirror?-1:1)*test.offset[0]/12,test.offset[1]/12);
        gl.uniform4f(uniform('u_shadow_color'),...test.color.map((value,index)=>index===3?value:value/255));
        gl.clearColor(0,0,0,0);gl.clear(gl.COLOR_BUFFER_BIT);gl.drawArrays(gl.TRIANGLES,0,6);
        const actual=new Uint8Array(96*64*4);gl.readPixels(0,0,96,64,gl.RGBA,gl.UNSIGNED_BYTE,actual);
        let maximum=0,total=0,activeChannels=0;
        for(let y=0;y<64;y++)for(let x=0;x<96;x++)for(let channel=0;channel<4;channel++) {
          const ref=(y*96+x)*4,alpha=reference[ref+3];
          const expected=channel===3?alpha:Math.round(reference[ref+channel]*alpha/255);
          const delta=Math.abs(actual[((63-y)*96+x)*4+channel]-expected);
          maximum=Math.max(maximum,delta);total+=delta;
          if(expected||actual[((63-y)*96+x)*4+channel])activeChannels++;
        }
        rows.push({...test,delta:maximum,meanActiveDelta:total/activeChannels});
      }
      const error = gl.getError();
      if (error !== gl.NO_ERROR) throw Error(`WebGL error ${error}`);
      return rows;
    }, {vertex:shader('VERTEX_SHADER'), fragment:shader('FRAGMENT_SHADER'), cases});
    const output = process.env.OUT;
    if (output) {
      fs.mkdirSync(output, {recursive:true});
      fs.writeFileSync(path.join(output, 'receipt.json'), JSON.stringify({sourcePath, rows}, null, 2));
    }
    for (const row of rows) assert.ok(row.delta <= (row.tolerance??1),
      `${row.name}: actual Rust shader matches native colour filtering ${JSON.stringify(row)}`);
    console.log(`${rows.length} canonical GPU colour-filter cases passed`);
  } finally {await browser.close()}
})().catch(error => {console.error(error); process.exitCode=1});
