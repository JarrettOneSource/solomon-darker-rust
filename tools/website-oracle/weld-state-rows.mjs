// Source physical and ownership rows shared by cast and contact recordings.
const point = value => value == null ? null : [value.x, value.y]
const registration = value => value ? [value.managerLane, value.registrationOrdinal] : null
const physicsKeys = ['alpha', 'scale', 'damage', 'remainingDamage', 'life', 'pulseSequence',
  'contactDue', 'impactDue', 'fallHeight', 'impactTicksRemaining', 'releaseAgeTicks',
  'contactsRemaining', 'headingDegrees', 'rotationDegrees', 'height', 'verticalVelocity',
  'collisionRadius', 'contactDamage', 'contactEnabled', 'lifetimeTicksRemaining', 'underpowered', 'hitTargetIds', 'boulderTerminalCharge']
physicsKeys.push('targetId', 'remainingTicks', 'flightTicks', 'variant', 'quantity', 'shellScale', 'assemblyScale')
export function weldActorRow(actor) {
  const physics = Object.fromEntries(physicsKeys.filter(key => key in actor).map(key => [key, actor[key]]))
  for (const key of ['direction', 'endpoint', 'midpoint', 'origin', 'position', 'velocity', 'start', 'end', 'delta']) {
    if (actor[key] !== undefined) physics[key] = point(actor[key])
  }
  if (actor.phase !== undefined) physics.phase = actor.phase
  if (actor.rocks) physics.rocks = actor.rocks.map(rock => [rock.rockId, rock.damageRemaining,
    [rock.localPosition.x, rock.localPosition.y, rock.localPosition.z], point(rock.releaseOffset), rock.phase, rock.decay])
  if (actor.debris && !Array.isArray(actor.debris)) {
    const debris = actor.debris
    physics.debris = [debris.alpha, debris.height, point(debris.position), point(debris.velocity), debris.verticalVelocity]
  }
  return { age: actor.ageTicks, build: actor.buildId ?? null, id: actor.id, kind: actor.kind,
    light: registration(actor.lightRegistration), painters: (actor.painterRegistrations ?? []).map(registration), physics }
}
