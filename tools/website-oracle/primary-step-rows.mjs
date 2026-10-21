// Complete source actor rows shared by single and multi-player acceptance.
export const point = p => p === null ? null : [p.x, p.y]
export const registration = r => r ? [r.managerLane, r.registrationOrdinal] : null
export function projectileRow(p) {
  const detail = p.kind === 'ether' ? [p.damageRetention, p.headingDegrees, p.piercesRemaining, p.reacquiresTarget,
    p.speed, p.targetId, p.turnInput, p.turnAccumulator, p.underpowered, p.visualScale]
    : p.kind === 'fire' ? [p.burnDamage, p.emberDamage, p.emberFragments, p.explodeDamage,
      p.explodeRadius, p.privateSeed, p.spentEmber, p.underpowered]
    : [p.assemblyCharge, p.hitTargetIds, p.maximumCharge, p.orientation, p.remainingDamage, p.shellCharge, p.toughness]
  return { kind: p.kind, id: p.id, age: p.ageTicks, phase: p.phase, position: point(p.position),
    velocity: point(p.velocity), direction: point(p.direction), charge: p.charge, damage: p.damage,
    flight: p.flightTicks, registration: registration(p.lightRegistration), detail }
}
export function transientRow(p) {
  let detail
  switch (p.kind) {
    case 'air': detail = [p.birthTick, point(p.direction), point(p.endpoint), p.hurricaneCharge,
      point(p.midpoint), point(p.origin), p.targetId, p.underpowered, p.variant]; break
    case 'water': detail = [point(p.direction), p.obstructionDistance, point(p.obstructionPoint),
      point(p.origin), p.speed, p.underpowered, p.variant]; break
    case 'earth-called-rock': detail = [p.fallVelocity, p.falling, p.height, p.lateralMagnitude,
      p.parentId, point(p.position), p.rotation, p.rotationStep, p.scale, p.speed, p.targetHeight, p.variant]; break
    case 'earth-impact': detail = [p.birthTick, p.charge, p.lifetimeTicks, point(p.origin)]; break
    case 'ether-impact': detail = [p.birthTick, point(p.origin), p.visualScale]; break
    case 'ether-blast': detail = [p.birthTick, p.charges, point(p.origin), p.presentationRng]; break
    case 'fire': detail = [point(p.direction), point(p.origin), p.variant]; break
    case 'fire-impact': detail = [point(p.origin)]; break
    default: throw new Error(`Add the actual ${p.kind} contract to this recorder`)
  }
  return { kind: p.kind, id: p.id, age: p.ageTicks, light: registration(p.lightRegistration),
    registrations: p.painterRegistrations.map(registration), detail }
}
