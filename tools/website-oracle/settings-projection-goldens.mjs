import { loadKernel, writeFixture } from './oracle.mjs'
const settings = await loadKernel('game-settings.ts')
const cameras = []
for (const nativeZoom of [1.2, 1.35]) {
  for (const percent of [0, 75, 100, 125, 200]) {
    cameras.push({ nativeZoom, percent, expected: settings.cameraZoomForFov(nativeZoom, percent) })
  }
}
const lights = [0, 24, 50, 100, 200].map(percent => ({ percent,
  expected: settings.gameLightQuality({ lightQualityPercent: percent }) }))
const defaults = settings.DEFAULT_GAME_SETTINGS
const controls = []
for (const action of settings.GAME_BINDING_ACTIONS) {
  for (const code of [defaults.controls[action], 'Escape', 'KeyQ', 'Mouse0', 'Mouse2', '', 'A'.repeat(64), 'A'.repeat(65), '😀'.repeat(32), '😀'.repeat(33)]) {
    controls.push({ action, code, expected: settings.rebindGameControl(defaults.controls, action, code) })
  }
}
const online = []
for (let bits = 0; bits < 32; bits += 1) {
  const input = { ...defaults,
    enableOnlineFeatures: Boolean(bits & 1), enableGlobalChat: Boolean(bits & 2),
    enableActivityMessages: Boolean(bits & 4), submitRunsToServer: Boolean(bits & 8),
    enableSharedHub: Boolean(bits & 16),
  }
  online.push({ bits, preferences: settings.gameOnlinePreferences(input), sharedHub: settings.gameSharedHubEnabled(input) })
}
const migrations = []
for (const missing of [['openCheats'], ['cycleHotbar'], ['openCheats', 'cycleHotbar']]) {
  for (const occupied of [[], ['Backquote'], ['KeyR'], ['KeyR', 'KeyV'],
    ['Backquote', 'KeyR', 'KeyV', 'F1', 'F2', 'F3', 'F4']]) {
    const controls = { ...defaults.controls }
    for (const action of missing) delete controls[action]
    // Give every populated action a distinct source-valid code, then occupy the
    // migration candidates without accidentally duplicating an existing binding.
    const retained = settings.GAME_BINDING_ACTIONS.filter(action => !missing.includes(action))
    for (const [index, action] of retained.entries()) controls[action] = `Custom${index}`
    for (const [index, code] of occupied.entries()) controls[retained[index]] = code
    const stored = { ...defaults, controls }
    const parsed = settings.readGameSettings({ getItem: () => JSON.stringify(stored) })
    migrations.push({ controls, expected: parsed.controls })
  }
}
writeFixture('settings-projection-goldens.json' , 'tools/website-oracle/settings-projection-goldens.mjs',
  'Current Website fa08ae249ff5563570bdb2a653e2ef9358ce7b83 game-settings.ts',
  { cameras, lights, defaults, controls, online, migrations })
