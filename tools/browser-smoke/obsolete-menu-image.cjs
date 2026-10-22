// A cancelled obsolete DOM image is a scene-lifecycle observation, not an
// HTTP failure. Retain the exact current sources; live-image cancellations remain errors.
module.exports = async function obsoleteMenuImage(page, request) {
  const path = new URL(request.url()).pathname;
  if (request.resourceType() !== 'image'
    || request.failure()?.errorText !== 'net::ERR_ABORTED'
    || !/\/assets\/stock\/menu\/(?:create-hand-(?:cupped|fist|raised)|menu-solomon-cloak-[0-4])\.png$/.test(path)) return null;
  try {
    const current = await page.evaluate(requested => ({
      referenced: [...document.images].some(image =>
        (image.src && new URL(image.src).pathname === requested)
        || (image.currentSrc && new URL(image.currentSrc).pathname === requested)),
      menu: document.querySelector('.game-menus')?.dataset.menuScreen,
      replacements: [...document.querySelectorAll('img.create-hand, img.title-solomon-cloak')].map(image => ({
        source: new URL(image.src).pathname, current: image.currentSrc && new URL(image.currentSrc).pathname,
        complete: image.complete, naturalWidth: image.naturalWidth,
      })),
    }), path);
    return current.referenced ? null : {path, failure: request.failure().errorText,
      reason: 'obsolete menu animation image; no DOM IMG references this URL', ...current};
  } catch { return null; }
};
