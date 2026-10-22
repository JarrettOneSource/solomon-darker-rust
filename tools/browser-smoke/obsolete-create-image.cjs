// A cancelled obsolete DOM image is a scene-lifecycle observation, not an
// HTTP failure. Retain the exact current sources; live-image cancellations remain errors.
module.exports = async function obsoleteCreateImage(page, request) {
  const path = new URL(request.url()).pathname;
  if (request.resourceType() !== 'image'
    || request.failure()?.errorText !== 'net::ERR_ABORTED'
    || !/\/assets\/stock\/menu\/create-hand-(cupped|fist|raised)\.png$/.test(path)) return null;
  try {
    const current = await page.evaluate(requested => ({
      referenced: [...document.images].some(image => new URL(image.src).pathname === requested),
      menu: document.querySelector('.game-menus')?.dataset.menuScreen,
      hands: [...document.querySelectorAll('img.create-hand')].map(image => ({
        source: new URL(image.src).pathname, current: image.currentSrc && new URL(image.currentSrc).pathname,
        complete: image.complete, naturalWidth: image.naturalWidth,
      })),
    }), path);
    return current.referenced ? null : {path, failure: request.failure().errorText,
      reason: 'obsolete Create pose image; no DOM IMG references this URL', ...current};
  } catch { return null; }
};
