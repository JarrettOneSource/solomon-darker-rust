// Read-only GPU observation for the ordinary admitted-mod/session-reset journey.
// Every wrapped browser call forwards its arguments and return value unchanged.
const assert = require('node:assert/strict');

async function observeGpuTextureLifetime(page) {
  await page.addInitScript(() => {
    const records = new Map();
    const identities = new WeakMap();
    const bindings = new WeakMap();
    let next = 0;
    const identify = (context, texture) => {
      if (!texture) return null;
      let row = identities.get(texture);
      if (!row) {
        row = { id: ++next, canvas: context.canvas?.id || 'loader', url: null, uploads: 0, deletes: 0, sampling: {} };
        identities.set(texture, row);
        records.set(row.id, row);
      }
      return row;
    };
    const prototype = WebGL2RenderingContext.prototype;
    const originalBind = prototype.bindTexture;
    prototype.bindTexture = function (target, texture) {
      const result = originalBind.call(this, target, texture);
      if (target === this.TEXTURE_2D) bindings.set(this, identify(this, texture));
      return result;
    };
    const originalUpload = prototype.texImage2D;
    prototype.texImage2D = function (...args) {
      const result = originalUpload.apply(this, args);
      const image = args.find(value => value instanceof HTMLImageElement);
      const row = bindings.get(this);
      if (row && image?.src.startsWith('blob:')) { row.url = image.src; row.uploads += 1; }
      return result;
    };
    const originalSampling = prototype.texParameteri;
    prototype.texParameteri = function (target, parameter, value) {
      const result = originalSampling.call(this, target, parameter, value);
      const row = bindings.get(this);
      if (target === this.TEXTURE_2D && row) row.sampling[parameter] = value;
      return result;
    };
    const originalDelete = prototype.deleteTexture;
    prototype.deleteTexture = function (texture) {
      const result = originalDelete.call(this, texture);
      const row = identities.get(texture);
      if (row) row.deletes += 1;
      return result;
    };
    Object.defineProperty(window, '__solomonGpuTextureLifetime', {
      value: () => structuredClone([...records.values()].filter(row => row.canvas === 'game' && row.url)),
    });
  });
}

function admittedTextureRows(trace, borrowedPaths) {
  assert.ok(borrowedPaths.length, 'the actual admitted session supplied borrowed mod-image witnesses');
  return borrowedPaths.map(url => {
    const rows = trace.filter(row => row.url === url);
    assert.equal(rows.length, 1, `${url}: world and UI share one actual GPU texture`);
    assert.equal(rows[0].uploads, 1, `${url}: the UI did not decode/upload the admitted image again`);
    return rows[0];
  });
}

function assertBorrowedTexturesRetired(trace, borrowedPaths) {
  for (const row of admittedTextureRows(trace, borrowedPaths)) {
    assert.equal(row.deletes, 1, `${row.url}: reset deletes through the world owner exactly once`);
  }
}

function assertBorrowedSamplingPreserved(before, after, borrowedPaths) {
  const previous = admittedTextureRows(before, borrowedPaths);
  const current = admittedTextureRows(after, borrowedPaths);
  for (let index = 0; index < previous.length; index += 1) {
    assert.deepEqual(current[index].sampling, previous[index].sampling,
      `${previous[index].url}: UI painting preserves the world owner's sampling`);
  }
}

module.exports = { observeGpuTextureLifetime, admittedTextureRows, assertBorrowedTexturesRetired, assertBorrowedSamplingPreserved };
