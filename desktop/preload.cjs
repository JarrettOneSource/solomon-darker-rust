const { contextBridge, ipcRenderer } = require('electron')

const endpoint = ipcRenderer.sendSync('solomon-dark:game-endpoint')
let beforeClose = async () => {}
ipcRenderer.on('solomon-dark:save-before-close', async (_event, id) => {
  try {
    await beforeClose()
    ipcRenderer.send('solomon-dark:saved', { id })
  } catch (error) {
    ipcRenderer.send('solomon-dark:saved', { id, error: error instanceof Error ? error.message : 'Saving failed.' })
  }
})

contextBridge.exposeInMainWorld('solomonDarkRuntime', Object.freeze({
  ...(endpoint ? { gameEndpoint: Object.freeze(endpoint) } : {}),
  desktop: Object.freeze({
    getState: () => ipcRenderer.invoke('solomon-dark:desktop-state'),
    startSession: options => ipcRenderer.invoke('solomon-dark:start-session', options),
    returnToLauncher: () => ipcRenderer.invoke('solomon-dark:launcher'),
    checkUpdates: () => ipcRenderer.invoke('solomon-dark:check-updates'),
    downloadUpdate: () => ipcRenderer.invoke('solomon-dark:download-update'),
    openWebsite: () => ipcRenderer.invoke('solomon-dark:open-website'),
    onBeforeClose: callback => {
      beforeClose = callback
      return () => { if (beforeClose === callback) beforeClose = async () => {} }
    },
    onState: callback => {
      const listener = (_event, state) => callback(state)
      ipcRenderer.on('solomon-dark:desktop-state', listener)
      return () => ipcRenderer.off('solomon-dark:desktop-state', listener)
    },
  }),
}))
