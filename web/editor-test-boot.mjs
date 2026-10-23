// The Website keeps its draft mounted; this document owns only a disposable test.
const origin = window.location.origin
let started = false
let cancelled = false
let cleanupTimer
const post = message => window.parent.postMessage(message, origin)
const cleanup = () => {
  if (cleanupTimer !== undefined) return
  // Hidden frames can suspend rAF. Only handshake/disposal is polled here;
  // gameplay remains on the ordinary frame loop and canonical native host.
  cleanupTimer = window.setInterval(() => {
    if (window.wasmBindings?.editor_test_cleanup?.()) {
      window.solomonLoader?.finish()
      window.clearInterval(cleanupTimer)
      cleanupTimer = undefined
    }
  }, 50)
}
window.addEventListener('message', async event => {
  if (event.origin !== origin || event.source !== window.parent) return
  const message = event.data
  if (!message || typeof message !== 'object') return
  if (message.type === 'solomon-editor-test-cancel') {
    cancelled = true
    if (window.__solomonEditorTest) window.__solomonEditorTest.cancelled = true
    cleanup()
    return
  }
  if (message.type !== 'solomon-editor-test-start' || started) return
  if (typeof message.name !== 'string' || typeof message.bytesBase64 !== 'string') return
  started = true
  window.__solomonEditorTest = { name: message.name, bytesBase64: message.bytesBase64, accountUsername: typeof message.accountUsername === 'string' ? message.accountUsername : null, cancelled }
  try {
    const entry = document.querySelector('script[data-editor-test-boot]')?.dataset.gameBoot
    if (!entry) throw new Error('The private test entry is missing.')
    await import(entry)
  } catch (error) {
    if (cleanupTimer !== undefined) {
      window.clearInterval(cleanupTimer)
      cleanupTimer = undefined
    }
    window.solomonLoader?.finish()
    post({ type: 'solomon-editor-test-state', state: 'error', message: error instanceof Error ? error.message : 'The private test could not load.' })
    post({ type: 'solomon-editor-test-disposed' })
  }
})
post({ type: 'solomon-editor-test-ready' })
