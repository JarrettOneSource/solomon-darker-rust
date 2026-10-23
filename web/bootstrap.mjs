// Platform module loading only. All visible startup and game UI is Rust GPU output.
const resources = __SOLOMON_RESOURCES__;
const resourceUrl = name => new URL(name, import.meta.url);
let loader;
let currentResource = resources.loaderModule;

try {
  loader = await import(resourceUrl(resources.loaderModule).href);
  currentResource = resources.loaderWasm;
  const loaderResponse = await fetch(resourceUrl(resources.loaderWasm));
  if (!loaderResponse.ok) throw new Error(`${resources.loaderWasm}: HTTP ${loaderResponse.status}`);
  await loader.default({ module_or_path: loaderResponse });
  window.solomonLoader = {
    progress: loader.progress,
    error: loader.error,
    finish: loader.finish,
    state: loader.gpu_ui_state,
    active: loader.active,
  };
  window.wasmBindings = { gpu_ui_state: loader.gpu_ui_state };
  await loader.boot_loader();
  loader.progress('Loading game code', resources.mainWasm, 0, 0);
  currentResource = resources.mainModule;
  const bindings = await import(resourceUrl(resources.mainModule).href);
  currentResource = resources.mainWasm;
  const response = await fetch(resourceUrl(resources.mainWasm));
  if (!response.ok) throw new Error(`${resources.mainWasm}: HTTP ${response.status}`);
  const wasm = await bindings.default({ module_or_path: response });
  window.wasmBindings = bindings;
  dispatchEvent(new CustomEvent('TrunkApplicationStarted', { detail: { wasm } }));
} catch (error) {
  const message = `${currentResource}: ${error instanceof Error ? error.message : String(error)}`;
  if (loader) loader.error(message);
  console.error(message);
}
