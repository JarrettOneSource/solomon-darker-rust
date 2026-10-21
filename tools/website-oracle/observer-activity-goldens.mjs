import { loadKernel, writeFixture } from './oracle.mjs'

const observer = await loadKernel('observer-activity.ts')

function offer(sequence, level = 2, options = [[8, 1], [65, 2]]) {
  return {
    automaticChoiceIndex: null,
    level,
    options: options.map(([skillId, targetRank]) => ({
      insight: false,
      skillId,
      targetRank,
      weldBuildId: null,
    })),
    sequence,
  }
}

function player(name, pendingOffer = null, learnedSkills = [], deferredSkillChoices = 0) {
  return {
    config: { displayName: name },
    progression: { deferredSkillChoices, learnedSkills, pendingOffer },
  }
}

const snapshot = (entries, tick = 100) => ({
  players: Object.fromEntries(entries),
  playerOrder: entries.map(([id]) => id),
  tick,
})
const cases = []
function add(name, before, after) {
  cases.push({
    name,
    before,
    after,
    offers: observer.observerSkillOffers(after),
    events: observer.deriveObserverSkillEvents(before, after),
  })
}

const empty = snapshot([])
const first = offer(7)
const second = offer(8, 3, [[72, 1], [8, 3]])
const before = snapshot([['p', player('Sirmin', first)]])

add('empty match', empty, empty)
add('new participant has offers without a previous-choice event', empty, before)
add('removed participant has no event', before, empty)
add('unchanged offer sequence', before, before)
add('changed options with the same sequence are not a new choice', before,
  snapshot([['p', player('Sirmin', offer(7, 20, [[82, 5]]))]]))
add('first offer for an existing participant', snapshot([['p', player('Sirmin')]]), before)
add('accepted offered skill', before, snapshot([['p', player('Sirmin', null, [[8, 1]])]], 201))
add('first increased option wins rather than greatest rank', before,
  snapshot([['p', player('Sirmin', null, [[8, 1], [65, 20]])]], 202))
add('offered skill order is retained',
  snapshot([['p', player('Sirmin', offer(7, 2, [[65, 1], [8, 1]]))]]),
  snapshot([['p', player('Sirmin', null, [[8, 10], [65, 1]])]], 203))
add('current learned rank is displayed after an increase',
  snapshot([['p', player('Sirmin', first, [[65, 1]])]]),
  snapshot([['p', player('Sirmin', null, [[65, 2]])]], 204))
add('a saved choice', before, snapshot([['p', player('Sirmin', null, [], 1)]], 205))
add('an accepted skill takes precedence over saving', before,
  snapshot([['p', player('Sirmin', null, [[8, 1]], 1)]], 206))
add('saving takes precedence over reroll text', before,
  snapshot([['p', player('Sirmin', second, [], 1)]], 207))
add('reroll resolves the old offer before opening its replacement', before,
  snapshot([['p', player('Sirmin', second)]], 208))
add('a closed choice with no rank change', before,
  snapshot([['p', player('Sirmin')]], 209))
add('an increase outside the previous offered skills is ignored', before,
  snapshot([['p', player('Sirmin', null, [[72, 4]])]], 210))
add('a rank decrease is not an accepted choice',
  snapshot([['p', player('Sirmin', first, [[8, 3]])]]),
  snapshot([['p', player('Sirmin', null, [[8, 2]])]], 211))
add('source names use the current participant snapshot', before,
  snapshot([['p', player('Renamed Wizard', null, [[8, 1]])]], 212))
add('unrecognized catalog IDs retain the source fallback name',
  snapshot([['p', player('Sirmin')]]),
  snapshot([['p', player('Sirmin', offer(0, 1, [[999, 0], [8, 1], [65, 2]]))]], 0))
add('all native skill names and rank suffixes',
  snapshot([['p', player('Sirmin')]]),
  snapshot([['p', player('Sirmin', offer(9, 30,
    Array.from({ length: 83 }, (_, id) => [id, id % 3])))]], 213))
add('participant insertion order and resolved-before-offered order',
  snapshot([
    ['z-last-alphabetically', player('Zorbin', first)],
    ['a-first-alphabetically', player('Alain', first)],
  ]),
  snapshot([
    ['z-last-alphabetically', player('Zorbin', second, [[8, 1]])],
    ['a-first-alphabetically', player('Alain', second, [], 1)],
    ['new', player('New Wizard', first)],
  ], 214))

writeFixture(
  'observer-activity-goldens.json',
  'tools/website-oracle/observer-activity-goldens.mjs',
  'Website/frontend/src/game/observer-activity.ts',
  { cases },
)
