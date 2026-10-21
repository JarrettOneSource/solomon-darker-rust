// The production multi-player entry point, including old actors without a current owner.
import { loadKernel, writeFixture } from './oracle.mjs'
import { point, projectileRow, transientRow } from './primary-step-rows.mjs'

const spells = await loadKernel('core-kernels/primary-spells.ts')
const character = await loadKernel('core-kernels/player-character.ts')
const progression = await loadKernel('core-kernels/player-progression.ts')
const primary = await loadKernel('core-kernels/native-primary-skill-profile.ts')
const random = await loadKernel('core-kernels/native-rng.ts')
const cases = []
for (const [name, elements] of [
  ['shared-flight-and-called-rocks', ['fire', 'earth', 'ether']],
  ['college-authority-omission', ['earth', 'water', 'air']],
]) {
  const ids = ['charlie', 'alpha', 'bravo']
  const definitions = elements.map((element, index) => ({
    id: ids[index], element, mana: index === 2 ? .05 : 100,
    position: { x: 100 + index * 70, y: 200 },
  }))
  const profiles = {}, retained = {}, mana = {}, targets = {}
  for (const [index, definition] of definitions.entries()) {
    const { id, element, position } = definition
    const config = { element, discipline: 'arcane', displayName: id }
    retained[id] = character.createPlayerCharacter(config, position)
    const book = progression.createPlayerSkillBook(config)
    profiles[id] = primary.nativePrimarySkillProfile(book, progression.playerStatBook(), { damage: 1, manaCost: 1 })
    mana[id] = definition.mana
    targets[id] = [{ id: `enemy:${index}`, position: { x: 450 + index * 100, y: -150 }, active: true,
      actorFlags: 2, attachment: { x: 0, y: -3 }, bodyRadius: 15, cellBindingOrder: index,
      headingDeg: 0, kind: 'enemy', nativePriority: 0, pendingRemove: false,
      queryLane: 'grid', registrationOrder: index }]
  }
  let state = spells.createPrimarySpellSimulation(), rng = random.createNativeRng(7)
  let actor = 49, transient = 3
  const frames = []
  for (let tick = 0; tick <= 160; tick += 1) {
    const activeIds = ids.filter(id => !(name === 'shared-flight-and-called-rocks' && (tick >= 155 || (id === 'charlie' && tick >= 85))))
    const players = Object.fromEntries(activeIds.map(id => [id, retained[id]]))
    const authority = Object.fromEntries(activeIds.filter(id => !(name === 'college-authority-omission' && id === 'charlie' && (tick < 20 || tick >= 65))).map(id => [id, {
      weaponKind: 'staff', alive: true, availableMana: mana[id], castProgressFactor: 1,
      eligible: tick < 150, planeActive: false, primarySkill: profiles[id],
    }]))
    const result = spells.stepPrimarySpells({
      canPlaceProjectile: spell => activeIds.includes(spell.ownerId),
      canTraverseProjectile: spell => activeIds.includes(spell.ownerId),
      castAuthority: authority,
      inputs: Object.fromEntries(activeIds.map((id, index) => [id, {
        aim: { x: 400, y: -100 }, cast: { primary: tick < [110, 120, 70][index] && !(name === 'college-authority-omission' && id === 'charlie' && tick === 20), quickbar: null },
        movement: { x: 0, y: 0 }, viewportWidth: 800, viewportHeight: 600,
      }])),
      players, previousPlayers: players,
      registerWorldPainter: managerLane => ({ managerLane, registrationOrdinal: managerLane === 'actor' ? actor++ : transient++ }),
      rng, spells: state, tick, viewScale: 1.2,
      spellObstructionPoint: () => null,
      spellRangeEndpoint: (_owner, start, direction) => ({ x: start.x + direction.x * 600, y: start.y + direction.y * 600 }),
      spellTargets: id => targets[id] ?? [],
      worldKeyForPlayer: id => `hub:${id === 'alpha' && tick >= 50 ? 'office' : 'courtyard'}`,
    })
    Object.assign(retained, result.players)
    for (const id of activeIds) mana[id] -= result.manaSpent[id]
    state = result.spells; rng = result.rng
    if (![0, 1, 14, 19, 20, 30, 49, 50, 64, 65, 70, 84, 85, 90, 110, 120, 121, 149, 150, 160].includes(tick)) continue
    frames.push({ tick, players: activeIds.map(id => {
      const player = retained[id], cast = player.primaryCast
      return { id, mana: mana[id], spent: result.manaSpent[id], underflow: result.manaUnderflowPlayerIds.includes(id),
        heading: player.headingIndex, cast: [cast.actionTick, point(cast.aimDirection), cast.castSequence,
          cast.channelActive, cast.emissionSequence, cast.etherBlastCharge, cast.etherBlastChargeCueSequence,
          cast.fizzleSequence, cast.held, cast.oneShotAttackPoseHeld, cast.selectedPrimaryAgeTicks,
          cast.selectedPrimaryId, cast.targetId, cast.underpowered, cast.weaponPulse] }
    }), state: { nextId: state.nextId,
      projectiles: state.projectiles.map(row => ({ ...projectileRow(row), owner: row.ownerId, world: row.worldKey })),
      transients: state.transients.map(row => ({ ...transientRow(row), owner: row.ownerId, world: row.worldKey })),
    }, rng, order: [actor, transient], emissions: result.channelEmissions.map(e => [e.ownerId, e.damage, point(e.direction), point(e.endpoint), e.id, e.kind, e.manaCost, point(e.origin), point(e.queryOrigin), e.underpowered]) })
  }
  cases.push({ name, definitions, frames })
}
writeFixture('primary-batch-goldens.json', 'primary-batch-goldens.mjs', [
  'core-kernels/primary-spells.ts',
], { cases })
