import {readFileSync} from 'node:fs'
import {resolve} from 'node:path'
import {gameDir,writeFixture} from './oracle.mjs'

const source = readFileSync(resolve(gameDir,'MainMenuScene.tsx'),'utf8')
const expression = source.match(/canInvite=\{Boolean\(([\s\S]+?)\)\}/)?.[1]
if(!expression)throw new Error('Source resolved-card Invite predicate not found')
// Execute the exact JSX prop expression rather than a second transcription.
const canInvite = new Function('session','runtimeSnapshot','partyState','resolvedPlayerCard',`return Boolean(${expression})`)
const invites=[]
for(const sessionKind of ['global-hub','private-college','standalone']) {
  for(const worldKind of ['hub','boneyard']) {
    for(const leader of [false,true]) {
      for(const activity of ['hub','boneyard']) {
        invites.push({sessionKind,worldKind,leader,activity,expected:canInvite(
          {sessionKind,playerId:'local'}, {world:{kind:worldKind}},
          {party:{leaderPlayerId:leader?'local':'other'}}, {activity},
        )})
      }
    }
  }
}
writeFixture('hosted-social-goldens.json','hosted-social-goldens.mjs',
  'MainMenuScene.tsx PlayerCardDialog canInvite expression (pinned ed0a2d59)',{invites})
