import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { gameDir, loadKernel, writeFixture } from './oracle.mjs'

const selection = await loadKernel('hub-player-selection.ts')
const base = [
  { id: 'local', position: [100, 100], region: 'courtyard' },
  { id: 'back', position: [200, 200], region: 'courtyard' },
  { id: 'front', position: [202, 220], region: 'courtyard' },
  { id: 'private', position: [200, 230], region: 'library' },
]
const number = value => typeof value === 'number' ? value : Number(value)
function snapshot(players) {
  return {
    players: Object.fromEntries(players.map(player => [player.id, {
      position: { x: player.position[0], y: player.position[1] },
    }])),
    world: {
      participants: Object.fromEntries(players.filter(player => player.region !== null)
        .map(player => [player.id, { region: player.region }])),
    },
  }
}
const pointer = []
function recordPointer(players, local, point) {
  pointer.push({ players, local, point, expected: selection.selectHubPlayerAtPoint(
    snapshot(players), local, { x: number(point[0]), y: number(point[1]) },
  ) })
}
// All four inclusive actor edges, just outside each edge, overlapping depth and
// registration-order ties run the public unchanged Source selector.
for (const point of [
  [100, 100], [201, 210], [200, 230], [157, 110], [247, 110],
  [202, 110], [202, 250], [156.999, 180], [247.001, 180],
  [202, 109.999], [202, 250.001], [235, 180], [260, 150],
  ['NaN', 200], [200, 'NaN'], ['Infinity', 200], [200, '-Infinity'],
]) recordPointer(base, 'local', point)
recordPointer(base, 'missing', [200, 200])
recordPointer(base.map(player => player.id === 'local' ? { ...player, region: null } : player),
  'local', [200, 200])
const tied = base.map(player => player.id === 'back'
  ? { ...player, position: [200, 220] } : player)
recordPointer(tied, 'local', [201, 210])
recordPointer([tied[0], tied[2], tied[1], tied[3]], 'local', [201, 210])
recordPointer(base.map(player => player.id === 'back'
  ? { ...player, position: [200, 240] } : player), 'local', [201, 220])

const nearest = []
const nearPlayers = base.map(player => player.id === 'back'
  ? { ...player, position: [180, 100] }
  : player.id === 'front' ? { ...player, position: [160, 100] } : player)
for (const players of [base, nearPlayers,
  nearPlayers.map(player => player.id === 'front'
    ? { ...player, position: [220, 100] } : player),
  nearPlayers.map(player => player.id === 'back'
    ? { ...player, position: [160, 100] } : player),
  nearPlayers.map(player => player.id === 'local' ? { ...player, region: null } : player),
]) {
  for (const maximum of [0, -1, 59.999, 60, 80, 120, 'NaN', 'Infinity']) {
    nearest.push({ players, local: 'local', maximum, expected: selection.nearestHubPlayer(
      snapshot(players), 'local', number(maximum),
    ) })
  }
}
nearest.push({ players: base, local: 'missing', maximum: 120,
  expected: selection.nearestHubPlayer(snapshot(base), 'missing') })

const hubSource = readFileSync(resolve(gameDir, 'HubScene.tsx'), 'utf8')
const cardSource = hubSource.slice(hubSource.indexOf('{selectedPlayerId &&'),
  hubSource.indexOf('{pickerOpen &&'))
const expression = cardSource.match(/canInvite=\{Boolean\(([\s\S]+?)\)\}/)?.[1]
if (!expression) throw new Error('Source local actor-card invite predicate not found')
const canInvite = new Function('partyState', 'playerId', 'alreadyTogether',
  `return Boolean(${expression})`)
const invites = []
for (const partyPresent of [false, true]) {
  for (const leader of [false, true]) {
    for (const alreadyTogether of [false, true]) {
      invites.push({ partyPresent, leader, alreadyTogether, expected: canInvite(
        partyPresent ? { party: { leaderPlayerId: leader ? 'local' : 'other' } } : null,
        'local', alreadyTogether,
      ) })
    }
  }
}
writeFixture('hub-player-selection-goldens.json', 'hub-player-selection-goldens.mjs',
  'Website hub-player-selection.ts public functions and HubScene local PlayerCardDialog canInvite prop',
  { pointer, nearest, invites })
