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
      const error = gl.getError();
      if (error !== gl.NO_ERROR) throw Error(`WebGL error ${error}`);
      return rows;
    }, {vertex:shader('VERTEX_SHADER'), fragment:shader('FRAGMENT_SHADER'), cases});
    const output = process.env.OUT;
    if (output) {
      fs.mkdirSync(output, {recursive:true});
      fs.writeFileSync(path.join(output, 'receipt.json'), JSON.stringify({sourcePath, rows}, null, 2));
    }
    for (const row of rows) assert.ok(row.delta <= 1,
      `${row.name}: actual Rust shader matches native colour filtering ${JSON.stringify(row)}`);
    console.log(`${rows.length} canonical GPU colour-filter cases passed`);
  } finally {await browser.close()}
})().catch(error => {console.error(error); process.exitCode=1});
