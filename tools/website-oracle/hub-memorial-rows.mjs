// Read-only source value projection shared by the portrait and actual-world fixtures.
const title = value => value[0].toUpperCase() + value.slice(1)
const garment = value => value === null ? null : {
  primary_tint: value.primaryTint, secondary_tint: value.secondaryTint, selector: value.selector,
}
export const memorialPortrait = value => value === null ? null : {
  account_username: value.accountUsername, awesomeness: value.awesomeness,
  awesomest_kill: value.awesomestKill, captured_at_tick: value.capturedAtTick,
  config: { name: value.config.displayName, element: title(value.config.element), discipline: title(value.config.discipline) },
  elapsed_ticks: value.elapsedTicks,
  equipment: { hat: garment(value.equipment.hat), robe: garment(value.equipment.robe),
    weapon: value.equipment.weapon === null ? null : { kind: title(value.equipment.weapon.kind), selector: value.equipment.weapon.selector } },
  heading_index: value.headingIndex, level: value.level, monsters_killed: value.monstersKilled,
  player_id: value.playerId, portrait_scale: value.portraitScale, run_id: value.runId, wave: value.wave,
}
export const memorialSnapshot = state => ({
  next_age: state.nextAge, next_portrait_id: state.nextPortraitId,
  slots: state.slots.map(slot => ({ age: slot.age, marker: slot.marker, portrait: memorialPortrait(slot.portrait), portrait_id: slot.portraitId })),
})
