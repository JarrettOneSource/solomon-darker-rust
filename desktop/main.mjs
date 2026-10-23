import { randomBytes } from 'node:crypto'
import { readFile } from 'node:fs/promises'
import { networkInterfaces } from 'node:os'
import { join, resolve } from 'node:path'
import { app, BrowserWindow, clipboard, dialog, ipcMain, Menu, net, protocol, session, shell } from 'electron'
import { startStaticClientServer } from './static-client-server.mjs'
import { startLocalGameHost } from './local-game-host.mjs'
import { createPeerInvite } from './peer-network.mjs'
import { startLocalPeer } from './local-peer.mjs'
import { checkForDesktopUpdate } from './updates.mjs'

const APP_ORIGIN = 'solomon-darker://app'
const LAUNCHER_URL = `${APP_ORIGIN}/__desktop/launcher.html`
let window
let clientServer
let gameHost
let peer
let endpoint = null
let invite = null
let mode = null
let build
let update = null
let updateError = null
let checking
let changingSession = false
let closing = false
let quitting = false
let pendingSave = null

app.setName('Solomon Darker')
if (process.env.SDR_DESKTOP_USER_DATA) app.setPath('userData', resolve(process.env.SDR_DESKTOP_USER_DATA))
protocol.registerSchemesAsPrivileged([{
  scheme: 'solomon-darker',
  privileges: { standard: true, secure: true, supportFetchAPI: true, corsEnabled: true, stream: true },
}])
app.commandLine.appendSwitch('enable-gpu-rasterization')

if (!app.requestSingleInstanceLock()) app.quit()
else {
  app.on('second-instance', () => { window?.show(); window?.focus() })
  app.on('before-quit', event => {
    if (quitting) return
    event.preventDefault()
    void exit()
  })
  void app.whenReady().then(start).catch(async error => {
    dialog.showErrorBox('Solomon Darker could not start', error.message)
    closing = true
    await shutdown()
    app.exit(1)
  })
}

async function start() {
  const applicationRoot = app.getAppPath()
  build = process.env.SDR_DESKTOP_BUILD_JSON
    ? JSON.parse(process.env.SDR_DESKTOP_BUILD_JSON)
    : JSON.parse(await readFile(join(applicationRoot, 'desktop-build.json'), 'utf8'))
  const clientRoot = resolve(process.env.SDR_DESKTOP_CLIENT_ROOT || join(applicationRoot, 'client'))
  clientServer = await startStaticClientServer({ root: clientRoot })
  protocol.handle('solomon-darker', request => {
    const url = new URL(request.url)
    if (url.host !== 'app') return new Response(null, { status: 404 })
    if (url.pathname.startsWith('/api/')) {
      return Response.json({ error: 'This feature is available on the Solomon Darker website.' }, { status: 503 })
    }
    return net.fetch(`${clientServer.origin}${url.pathname}${url.search}`, { method: request.method })
  })
  session.defaultSession.setPermissionRequestHandler((_contents, _permission, callback) => callback(false))
  session.defaultSession.setPermissionCheckHandler(() => false)
  window = new BrowserWindow({
    backgroundColor: '#0c0910', height: 850, minHeight: 600, minWidth: 960, show: false,
    title: 'Solomon Darker', width: 1360,
    webPreferences: {
      contextIsolation: true, nodeIntegration: false, preload: join(applicationRoot, 'preload.cjs'),
      sandbox: true, webSecurity: true,
    },
  })
  window.webContents.setWindowOpenHandler(() => ({ action: 'deny' }))
  window.webContents.on('will-navigate', (event, url) => { if (!isAppUrl(url)) event.preventDefault() })
  window.webContents.on('will-attach-webview', event => event.preventDefault())
  window.on('close', event => { if (!quitting) { event.preventDefault(); void exit() } })
  window.once('ready-to-show', () => window.show())
  ipcMain.on('solomon-dark:game-endpoint', event => { event.returnValue = trusted(event) ? endpoint : null })
  ipcMain.handle('solomon-dark:desktop-state', event => { requireTrusted(event); return state() })
  ipcMain.handle('solomon-dark:start-session', async (event, options) => {
    requireTrusted(event)
    if (window.webContents.getURL() !== LAUNCHER_URL || changingSession) throw new Error('Return to the launcher first.')
    changingSession = true
    try {
      await startSession(options, applicationRoot)
      await window.loadURL(`${APP_ORIGIN}/game`)
    } catch (error) {
      await stopSession()
      throw error
    } finally { changingSession = false }
  })
  ipcMain.handle('solomon-dark:check-updates', event => { requireTrusted(event); return checkUpdates() })
  ipcMain.handle('solomon-dark:download-update', event => {
    requireTrusted(event)
    if (!update) throw new Error('No update is available.')
    return shell.openExternal(update.url)
  })
  ipcMain.handle('solomon-dark:open-website', event => { requireTrusted(event); return shell.openExternal('https://solomondarker.com') })
  ipcMain.handle('solomon-dark:launcher', async event => { requireTrusted(event); await returnToLauncher() })
  ipcMain.on('solomon-dark:saved', (event, result) => {
    if (trusted(event) && pendingSave && result?.id === pendingSave.id) {
      if (typeof result.error === 'string') pendingSave.reject(new Error(result.error))
      else pendingSave.resolve()
    }
  })
  installMenu()
  await window.loadURL(LAUNCHER_URL)
  void checkUpdates()
  setInterval(() => { void checkUpdates() }, 4 * 60 * 60 * 1000).unref()
}

async function startSession(options, applicationRoot) {
  if (!options || !['solo', 'host', 'join'].includes(options.mode)) throw new Error('Choose how to play.')
  if (options.mode === 'join') {
    peer = await startLocalPeer({ applicationRoot, mode: 'join', invite: options.invite, revision: build.revision, onExit: peerExited })
    endpoint = { kind: 'localhost', sessionKind: 'standalone', url: `ws://127.0.0.1:${peer.port}/game`, credential: peer.credential }
  } else {
    const credential = randomBytes(32).toString('base64url')
    if (options.mode === 'host') {
      createPeerInvite({ host: options.host, port: options.port, revision: build.revision, secret: credential })
    }
    gameHost = await startLocalGameHost({
      applicationRoot, origin: APP_ORIGIN, credential, revision: build.revision,
      onExit: code => {
        if (!quitting) dialog.showErrorBox('The local game stopped', `The game host exited (${code}). Your last saved checkpoint is still on this device.`)
      },
    })
    endpoint = gameHost.endpoint
    if (options.mode === 'host') {
      peer = await startLocalPeer({ applicationRoot, mode: 'host', authorityPort: Number(new URL(endpoint.url).port), port: options.port, secret: credential, revision: build.revision, onExit: peerExited })
      invite = createPeerInvite({ host: options.host, port: peer.port, secret: credential, revision: build.revision })
    }
  }
  mode = options.mode
  installMenu()
}

function state() {
  const addresses = Object.values(networkInterfaces()).flat().filter(address => address?.family === 'IPv4' && !address.internal).map(address => address.address)
  return { build, mode, addresses, update, updateError }
}

async function checkUpdates() {
  if (checking) return checking
  checking = (async () => {
    try {
      update = await checkForDesktopUpdate({ build, platform: process.platform, arch: process.arch, request: net.fetch.bind(net) })
      updateError = null
    } catch {
      updateError = 'Updates could not be checked. You can keep playing offline.'
    }
    if (!window.isDestroyed()) window.webContents.send('solomon-dark:desktop-state', state())
    installMenu()
    return state()
  })().finally(() => { checking = null })
  return checking
}

function installMenu() {
  if (!window || window.isDestroyed()) return
  Menu.setApplicationMenu(Menu.buildFromTemplate([
    ...(process.platform === 'darwin' ? [{ role: 'appMenu' }] : []),
    { label: 'Game', submenu: [
      { label: 'Save and return to launcher', enabled: mode !== null, click: () => { void returnToLauncher() } },
      { label: 'Copy multiplayer invite', enabled: invite !== null, click: () => {
        clipboard.writeText(invite)
      } },
      { type: 'separator' }, { role: 'quit' },
    ] },
    { role: 'editMenu' },
    { label: 'View', submenu: [{ role: 'togglefullscreen' }] },
    { label: 'Help', submenu: [
      { label: update ? 'Download available update' : 'Check for updates', click: () => { void manualUpdateCheck() } },
      { label: 'Solomon Darker website', click: () => { void shell.openExternal('https://solomondarker.com') } },
    ] },
  ]))
}

async function manualUpdateCheck() {
  await checkUpdates()
  const result = await dialog.showMessageBox(window, {
    type: 'info', message: update ? 'An update is available' : updateError || 'You have the latest available build.',
    detail: update ? 'Download the new app and replace this copy after saving and closing. Your saves stay on this device.' : '',
    buttons: update ? ['Download update', 'Later'] : ['OK'], cancelId: update ? 1 : 0,
  })
  if (update && result.response === 0) await shell.openExternal(update.url)
}

async function saveBeforeClose() {
  if (!mode || window.isDestroyed()) return true
  try {
    await new Promise((resolveSave, reject) => {
      const timer = setTimeout(() => reject(new Error('The game did not finish saving.')), 20_000)
      pendingSave = {
        id: randomBytes(16).toString('hex'),
        resolve: () => { clearTimeout(timer); resolveSave() },
        reject: error => { clearTimeout(timer); reject(error) },
      }
      window.webContents.send('solomon-dark:save-before-close', pendingSave.id)
    })
    return true
  } catch (error) {
    const result = await dialog.showMessageBox(window, {
      type: 'warning', message: 'The latest progress could not be saved', detail: error.message,
      buttons: ['Keep playing', 'Close without saving'], defaultId: 0, cancelId: 0,
    })
    return result.response === 1
  } finally { pendingSave = null }
}

async function returnToLauncher() {
  if (closing || changingSession) return
  closing = true
  try {
    if (!await saveBeforeClose()) return
    await window.loadURL('about:blank')
    await stopSession()
    await window.loadURL(LAUNCHER_URL)
    installMenu()
  } finally { closing = false }
}

async function exit() {
  if (closing) return
  closing = true
  if (window && !await saveBeforeClose()) { closing = false; return }
  await shutdown()
  quitting = true
  app.quit()
}

async function stopSession() {
  await peer?.close()
  peer = null
  await gameHost?.close()
  gameHost = null
  endpoint = null
  invite = null
  mode = null
}

async function shutdown() {
  // Retire the saved document before removing the services it still uses.
  if (window && !window.isDestroyed()) {
    try { await window.loadURL('about:blank') }
    catch {
      // A close no longer needs its renderer if navigation was interrupted.
      if (!window.isDestroyed()) window.destroy()
    }
  }
  await stopSession()
  await clientServer?.close()
}

function isAppUrl(value) {
  const url = new URL(value)
  return url.protocol === 'solomon-darker:' && url.host === 'app'
}

function trusted(event) {
  return event.sender === window.webContents && event.senderFrame !== null
    && event.senderFrame === window.webContents.mainFrame && isAppUrl(event.senderFrame.url)
}

function requireTrusted(event) {
  if (!trusted(event)) throw new Error('This action is available only in the desktop app.')
}

function peerExited(code) {
  if (!quitting) dialog.showErrorBox('The peer connection stopped', `The network service exited (${code}). Return to the launcher to reconnect.`)
}
