// Read-only GPU observations locate widgets; every interaction uses browser input.
const assert = require('node:assert/strict');
const {watchErrors,watchRequestFailures,settleErrors,observeDocumentReplacement,observeFrameRemoval}=require('./browser-errors.cjs');

async function readUi(page) {
  return page.evaluate(() => {
    const read = window.wasmBindings?.gpu_ui_state;
    if (!read) return null;
    return JSON.parse(read());
  });
}

async function waitForUi(page, predicate, description, { timeout = 45000 } = {}) {
  const deadline = Date.now() + timeout;
  let last;
  while (Date.now() < deadline) {
    last = await readUi(page);
    if (last?.error) throw new Error(`GPU UI failed: ${last.error}`);
    if (last && predicate(last)) return last;
    await page.waitForTimeout(50);
  }
  throw new Error(`Timed out waiting for ${description}: ${JSON.stringify(last)}`);
}

function findWidget(ui, id) {
  const matches = ui.widgets.filter(widget => widget.id === id);
  assert.ok(matches.length <= 1, `GPU widget ID is unique: ${id}`);
  return matches[0];
}

async function widget(page, id, {enabled = false, ...options} = {}) {
  const ui = await waitForUi(page, ui => {
    const target = findWidget(ui, id);
    return target && (!enabled || target.enabled === true);
  }, `${enabled ? 'enabled ' : ''}GPU widget ${id}`, options);
  return findWidget(ui, id);
}

async function noWidget(page, id, options) {
  return waitForUi(page, ui => !findWidget(ui, id), `GPU widget ${id} to close`, options);
}

async function byLabel(page, label, {role, prefix = '', timeout} = {}) {
  const matches = snapshot => snapshot.widgets.filter(widget => widget.id.startsWith(prefix)
    && (!role || widget.role === role)
    && (typeof label === 'string' ? widget.label === label : label.test(widget.label)));
  const snapshot = await waitForUi(page, snapshot => matches(snapshot).length > 0,
    `GPU label ${label}`, {timeout});
  const found = matches(snapshot);
  assert.equal(found.length, 1, `GPU label is unambiguous within ${prefix}: ${label}`);
  return found[0];
}

async function surface(page, name, options) {
  return waitForUi(page, ui => ui.surface === name, `GPU surface ${name}`, options);
}

async function ready(page, options) {
  return waitForUi(page, ui => ui.ready === true && ui.loading === null
    && ui.draw.meshes > 0 && ui.draw.triangles > 0 && ui.draw.textures > 0,
  'complete assets and an actual GPU UI draw', { timeout: 240000, ...options });
}

function bounds(target, {paint = false} = {}) {
  const [x, y, width, height] = paint ? target.paintRect : target.rect;
  return { x, y, width, height };
}

function center(bounds) {
  assert.ok([bounds.x, bounds.y, bounds.width, bounds.height].every(Number.isFinite)
    && bounds.width > 0 && bounds.height > 0, `visible GPU bounds: ${JSON.stringify(bounds)}`);
  return { x: bounds.x + bounds.width / 2, y: bounds.y + bounds.height / 2 };
}

const interactiveRoles=new Set(['button','textbox','password','input','text-entry','slider',
  'checkbox','switch','combobox','radio','tab','menuitem']);

async function actionable(page,id,{timeout=45000}={}) {
  const deadline=Date.now()+timeout;
  let previous;
  let last;
  while(Date.now()<deadline) {
    const snapshot=await readUi(page);
    if(snapshot?.error)throw new Error(`GPU UI failed: ${snapshot.error}`);
    const target=snapshot&&findWidget(snapshot,id);
    if(target?.enabled&&target.rect.every(Number.isFinite)
      &&target.rect[2]>0&&target.rect[3]>0) {
      const point=center(bounds(target));
      assert.ok(Number.isFinite(target.z),'actual widget hit order is available');
      const covering=snapshot.widgets.filter(widget=>widget.id!==id
        &&interactiveRoles.has(widget.role)&&widget.z>target.z
        &&point.x>=widget.rect[0]&&point.x<widget.rect[0]+widget.rect[2]
        &&point.y>=widget.rect[1]&&point.y<widget.rect[1]+widget.rect[3]);
      const stable=previous&&snapshot.frame!==previous.frame
        &&target.rect.every((value,index)=>value===previous.rect[index]);
      if(stable&&covering.length===0)return {target,point};
      previous={frame:snapshot.frame,rect:target.rect};
      last={target,covering:covering.map(widget=>widget.id),frame:snapshot.frame};
    } else {
      previous=undefined;
      last={target,surface:snapshot?.surface};
    }
    // Observe actual consecutive GPU frames, as a DOM locator waits for a
    // stable hit target. No fixed animation delay or hidden action is used.
    await nextFrame(page);
  }
  throw new Error(`GPU widget ${id} never became stable and unobstructed: ${JSON.stringify(last)}`);
}

async function click(page, id, options) {
  const {point}=await actionable(page,id);
  await page.mouse.click(point.x, point.y, options);
}

async function tap(page, id) {
  const {point}=await actionable(page,id);
  await page.touchscreen.tap(point.x, point.y);
}

async function fill(page, id, text, { touch = false } = {}) {
  if (touch) await tap(page, id);
  else await click(page, id);
  await focused(page, id);
  await page.keyboard.press('ControlOrMeta+A');
  // One replacement preserves the previous accepted draft when native validation rejects it.
  if (text) await page.keyboard.insertText(text);
  else await page.keyboard.press('Backspace');
  await nextFrame(page);
}

async function value(page, id, expected, options) {
  return waitForUi(page, ui => findWidget(ui, id)?.value === expected,
    `GPU widget ${id} value ${JSON.stringify(expected)}`, options);
}

async function focused(page, id, options) {
  return waitForUi(page, ui => findWidget(ui, id)?.focused === true,
    `GPU focus on ${id}`, options);
}

async function setSlider(page, id, target) {
  await click(page, id);
  await focused(page, id);
  await nextFrame(page);
  const current = Number((await widget(page, id)).value);
  assert.ok(Number.isFinite(current), `the actual slider reports its accepted number: ${id}`);
  const key = current < target ? 'ArrowRight' : 'ArrowLeft';
  for (let count = 0; count < Math.abs(target - current); count += 1) {
    await page.keyboard.press(key);
    await nextFrame(page);
  }
  await value(page, id, String(target));
}

async function wheel(page, id, deltaX, deltaY) {
  const point = center(bounds(await widget(page, id)));
  await page.mouse.move(point.x, point.y);
  await page.mouse.wheel(deltaX, deltaY);
}

async function drag(page, fromId, toId, { touch = false } = {}) {
  const from=(await actionable(page,fromId)).point;
  const to=(await actionable(page,toId)).point;
  if (!touch) {
    await page.mouse.move(from.x, from.y);
    await page.mouse.down();
    await page.mouse.move(to.x, to.y, { steps: 12 });
    await page.mouse.up();
    return;
  }
  const input = await page.context().newCDPSession(page);
  try {
    await input.send('Input.dispatchTouchEvent', {
      type: 'touchStart', touchPoints: [{ ...from, id: 1 }],
    });
    for (let step = 1; step <= 12; step += 1) {
      await input.send('Input.dispatchTouchEvent', { type: 'touchMove', touchPoints: [{
        x: from.x + (to.x - from.x) * step / 12,
        y: from.y + (to.y - from.y) * step / 12, id: 1,
      }] });
    }
  } finally {
    await input.send('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] });
    await input.detach();
  }
}

async function assertGpuOnly(page) {
  const evidence = await page.evaluate(() => {
    const passive = new Set(['CANVAS', 'SCRIPT', 'STYLE', 'LINK', 'NOSCRIPT']);
    const nonvisual = element => {
      const bounds = element.getBoundingClientRect();
      const style = getComputedStyle(element);
      return element.hidden || style.display === 'none' || style.visibility === 'hidden'
        || style.opacity === '0' || bounds.width === 0 || bounds.height === 0
        || bounds.right <= 0 || bounds.bottom <= 0
        || bounds.left >= innerWidth || bounds.top >= innerHeight
        || /^rect\(0px,? 0px,? 0px,? 0px\)$/.test(style.clip)
        || style.clipPath === 'inset(50%)';
    };
    const unexpected = [...document.body.querySelectorAll('*')].filter(element => {
      if (passive.has(element.tagName)) return false;
      if (element.tagName === 'AUDIO' && element.hidden && !element.controls) return false;
      if (element.hasAttribute('data-gpu-announcer')) {
        const style = getComputedStyle(element);
        return !['DIV', 'SPAN'].includes(element.tagName) || !nonvisual(element)
          || !['polite', 'assertive'].includes(element.getAttribute('aria-live'))
          || element.children.length !== 0 || element.hasAttribute('tabindex')
          || element.hidden || element.getAttribute('aria-hidden') === 'true'
          || style.display === 'none' || style.visibility === 'hidden'
          || element.hasAttribute('data-menu-action') || element.hasAttribute('data-panel-action');
      }
      const textAdapter = element.hasAttribute('data-gpu-text-input')
        && (element.tagName === 'TEXTAREA' || (element.tagName === 'INPUT'
          && ['text', 'search', 'email', 'password', 'url', 'tel', 'number'].includes(element.type)));
      const adapter = textAdapter || (element.tagName === 'INPUT' && element.type === 'file');
      if (adapter) return !nonvisual(element);
      return true;
    }).map(element => ({ tag: element.tagName, id: element.id, class: element.className }));
    const text = [...document.body.childNodes]
      .filter(node => node.nodeType === Node.TEXT_NODE && node.textContent.trim())
      .map(node => node.textContent.trim());
    return { unexpected, text, canvases: [...document.querySelectorAll('canvas')]
      .map(canvas => ({ id: canvas.id, width: canvas.width, height: canvas.height })) };
  });
  assert.deepEqual(evidence.unexpected, [], 'game has no visual DOM or hidden legacy controls');
  assert.deepEqual(evidence.text, [], 'game has no DOM text facade');
  assert.ok(evidence.canvases.some(canvas => canvas.id === 'game'
    && canvas.width > 0 && canvas.height > 0), 'the game owns a drawable canvas');
  const ui = await readUi(page);
  assert.ok(ui && ui.draw.meshes > 0 && ui.draw.triangles > 0 && ui.draw.textures > 0,
    'actual GPU UI meshes, triangles and textures were submitted');
  const ids = ui.widgets.map(widget => widget.id);
  assert.equal(new Set(ids).size, ids.length, 'all visible GPU widget IDs are unique');
  return { ...evidence, gpu: ui };
}

// Observe actual WebGL submissions while forwarding the original call unchanged.
async function observeGpu(page) {
  await page.addInitScript(() => {
    const counts = { calls: 0, vertices: 0, canvasCalls: {} };
    Object.defineProperty(window, '__solomonGpuDraws', { value: counts });
    for (const type of [WebGLRenderingContext, WebGL2RenderingContext]) {
      for (const [name, countIndex] of [['drawArrays', 2], ['drawElements', 1]]) {
        const draw = type.prototype[name];
        type.prototype[name] = function (...args) {
          const result = Reflect.apply(draw, this, args);
          const count = args[countIndex];
          if (count > 0) {
            counts.calls += 1;
            counts.vertices += count;
            const id = this.canvas instanceof HTMLCanvasElement
              && this.canvas.hasAttribute('data-gpu-loader')
              ? '[data-gpu-loader]' : this.canvas.id || '(unattached canvas)';
            counts.canvasCalls[id] = (counts.canvasCalls[id] || 0) + 1;
          }
          return result;
        };
      }
    }
  });
}

async function gpuDraws(page) {
  return page.evaluate(() => window.__solomonGpuDraws);
}

async function observeAnnouncements(page, {forceFallback = false} = {}) {
  await page.addInitScript(({forceFallback}) => {
    const announcements = [];
    Object.defineProperty(window, '__solomonAnnouncements', {value:announcements});
    const nativeAvailable = typeof document.ariaNotify === 'function';
    Object.defineProperty(window, '__solomonAnnouncementPlatform', {value:{nativeAvailable,forceFallback}});
    if(forceFallback)for(const type of [Document,Element]) {
      Object.defineProperty(type.prototype,'ariaNotify',{configurable:true,value:undefined});
    }
    const ownership = [];
    Object.defineProperty(window, '__solomonAnnouncementOwnership', {value:ownership});
    // Native ariaNotify is called while Rust is painting. Observe the actual
    // temporary canvas without reentering a WASM getter from its own callback.
    const active = () => Boolean(document.querySelector('canvas[data-gpu-loader]'));
    const sample = () => {
      const shared = document[Symbol.for('solomon.graphics.announcements')];
      const current = {owners:shared?.owners??0,loaders:shared?.loaders??0,
        count:document.querySelectorAll('[data-gpu-announcer]').length};
      if(JSON.stringify(ownership.at(-1))!==JSON.stringify(current))ownership.push(current);
      requestAnimationFrame(sample);
    };
    requestAnimationFrame(sample);
    for (const type of [Document, Element]) {
      const notify = type.prototype.ariaNotify;
      if (typeof notify !== 'function') continue;
      type.prototype.ariaNotify = function (text, options) {
        const result = Reflect.apply(notify, this, [text, options]);
        announcements.push({source:'ariaNotify',text,loaderActive:active()});
        return result;
      };
    }
    const observe = () => {
      const observer = new MutationObserver(records => {
        for (const record of records) {
          const element = record.target.nodeType === Node.ELEMENT_NODE
            ? record.target : record.target.parentElement;
          const announcer = element?.closest('[data-gpu-announcer]');
          // textContent replaces text nodes. Added nodes retain each real write;
          // the element already holds a later value when this callback is delivered.
          const text=record.type==='childList'
            ?Array.from(record.addedNodes,node=>node.textContent||'').join('')
            :record.target.textContent;
          if (announcer&&text) announcements.push({source:'aria-live',text,
            currentText:announcer.textContent,loaderActive:active()});
        }
      });
      observer.observe(document.body, {subtree:true,childList:true,characterData:true});
    };
    if (document.body) observe();
    else document.addEventListener('DOMContentLoaded', observe, {once:true});
  }, {forceFallback});
}

async function announcements(page) {
  return page.evaluate(() => window.__solomonAnnouncements);
}

async function announced(page, text, {afterIndex = 0, timeout = 10000} = {}) {
  await page.waitForFunction(({text,afterIndex}) => window.__solomonAnnouncements
    .slice(afterIndex).some(event => event.text.toLowerCase().includes(text.toLowerCase())),
  {text,afterIndex}, {timeout});
}

// Native Gamepad prototypes feed the production navigator adapter, never game commands.
async function installController(page) {
  await page.evaluate(() => {
    window.gpuTestPad = { buttons: [], axes: [0,0,0,0], analog: {}, reads: 0, connected: true };
    const gamepad = Object.create(Gamepad.prototype, {
      index: { value: 0 }, connected: { get: () => window.gpuTestPad.connected },
      mapping: { value: 'standard' },
      axes: { get: () => window.gpuTestPad.axes },
      buttons: { get: () => Array.from({ length: 17 }, (_, index) =>
        Object.create(GamepadButton.prototype, {
          pressed: { value: window.gpuTestPad.buttons.includes(index) },
          value: { value: window.gpuTestPad.analog[index]
            ?? (window.gpuTestPad.buttons.includes(index) ? 1 : 0) },
        })) },
    });
    Object.defineProperty(navigator, 'getGamepads', { value: () => {
      window.gpuTestPad.reads += 1;
      return [gamepad];
    } });
  });
}

async function controller(page, buttons = [], x = 0, y = 0,
  {rightX = 0, rightY = 0, analog = {}} = {}) {
  await page.evaluate(({ buttons, axes, analog }) => {
    window.gpuTestPad = { ...window.gpuTestPad, buttons, axes, analog };
  }, { buttons, axes:[x,y,rightX,rightY], analog });
  await page.evaluate(() => new Promise(resolve => requestAnimationFrame(resolve)));
}

async function pressController(page, button) {
  // Release in the browser after one actual sample. Waiting for a Node assertion
  // before releasing can turn a single press into repeated navigation on slow CI.
  await page.evaluate(button => new Promise((resolve, reject) => {
    const pad = window.gpuTestPad;
    let reads = pad.reads;
    let released = false;
    const timeout = setTimeout(() => {
      cancelAnimationFrame(frame);
      pad.buttons = [];
      reject(new Error('The game did not sample the controller press and release.'));
    }, 45000);
    const sample = () => {
      if (pad.reads > reads) {
        if (released) {
          clearTimeout(timeout);
          resolve();
          return;
        }
        pad.buttons = [];
        released = true;
        reads = pad.reads;
      }
      frame = requestAnimationFrame(sample);
    };
    pad.buttons = [button];
    let frame = requestAnimationFrame(sample);
  }), button);
}

async function nextFrame(page) {
  await page.evaluate(() => new Promise(resolve =>
    requestAnimationFrame(() => requestAnimationFrame(resolve))));
}

async function useNativeFocus(...pages) {
  for(const page of pages) {
    await page.evaluate(()=>{
      const events=[];
      Object.defineProperty(window,'__solomonFocusEvents',{value:events});
      for(const type of ['focus','blur'])addEventListener(type,event=>
        events.push({type:event.type,trusted:event.isTrusted}));
    });
    const client=await page.context().newCDPSession(page);
    try {
      // Playwright otherwise forces document.hasFocus() true in background
      // tabs. Restore Chrome's native focus before switching actual targets.
      await client.send('Emulation.setFocusEmulationEnabled',{enabled:false});
    } finally {await client.detach()}
  }
}

module.exports = {
  readUi, waitForUi, findWidget, widget, noWidget, byLabel, surface, ready, bounds, center, actionable, click, tap,
  fill, value, focused, setSlider, wheel, drag, assertGpuOnly, watchErrors, installController, controller, pressController, nextFrame,
  observeGpu, gpuDraws, watchRequestFailures, settleErrors, observeDocumentReplacement,observeFrameRemoval,
  observeAnnouncements, announcements, announced, useNativeFocus,
};
