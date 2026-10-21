const desktop = window.solomonDarkRuntime.desktop
const status = document.querySelector('#status')
const host = document.querySelector('#host')

function present(state) {
  document.querySelector('#version').textContent = `Build ${state.build.revision.slice(0, 8)} · Saves stored on this device`
  document.querySelector('#update').hidden = !state.update
}

const initial = await desktop.getState()
host.value = initial.addresses[0] || ''
present(initial)
desktop.onState(present)

async function launch(options) {
  for (const button of document.querySelectorAll('button')) button.disabled = true
  status.textContent = options.mode === 'join' ? 'Connecting to your friend…' : 'Starting your game…'
  try {
    await desktop.startSession(options)
  } catch (error) {
    status.textContent = error.message.replace(/^Error invoking remote method '[^']+': Error: /, '')
    for (const button of document.querySelectorAll('button')) button.disabled = false
  }
}

document.querySelector('#solo').addEventListener('click', () => { void launch({ mode: 'solo' }) })
document.querySelector('#host-form').addEventListener('submit', event => {
  event.preventDefault()
  void launch({ mode: 'host', host: host.value.trim(), port: Number(document.querySelector('#port').value) })
})
document.querySelector('#join-form').addEventListener('submit', event => {
  event.preventDefault()
  void launch({ mode: 'join', invite: document.querySelector('#invite').value.trim() })
})
document.querySelector('#download').addEventListener('click', () => { void desktop.downloadUpdate() })
document.querySelector('#website').addEventListener('click', () => { void desktop.openWebsite() })
document.querySelector('#check').addEventListener('click', async () => {
  status.textContent = 'Checking for updates…'
  const state = await desktop.checkUpdates()
  status.textContent = state.updateError || (state.update ? 'An update is ready to download.' : 'You have the latest available build.')
})
