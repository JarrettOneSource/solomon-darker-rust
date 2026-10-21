#!/usr/bin/env node
// Constructor tables and source-order flags, including the deliberately rejected dormant
// skeleton policies. Expected values come from the unchanged Website evaluator.
import { loadKernel, writeFixture } from './oracle.mjs'
const config = await loadKernel('core-kernels/boneyard-enemy-config.ts')
const schema = await loadKernel('core-kernels/boneyard-wave-schema.ts')

const recipe = (family) => ({
  uid:19, name:'oracle-recipe', classification:'miniboss', maximumHealth:123,
  primaryDamage:17, secondaryDamage:9, tertiaryDamage:0, extraDamage:0,
  attackSpeed:1.25,chaseSpeed:0.75,movementScale:1.5,experienceBonus:4,
  archerAccuracyMode:0, family, onDeathProgram:'miniboss-die',
  lootPolicies:{gold:1,item:2,orb:3,potion:4,powerup:5,specificItem:42},
})
const recipes = [
  ['PORTAL',recipe({kind:'portal',frequency:4})],
  ['SKELETON',recipe({kind:'skeleton',armor:true,headgear:5,weapon:'pike'})],
  ['SKELETONARCHER',{...recipe({kind:'archer',arrowType:'poison',extraArrows:3,headgear:4,multiArrowMode:2,rangeMode:3,strafing:true}),archerAccuracyMode:3}],
  ['ZOMBIE',recipe({kind:'zombie',bodyType:1,flyblown:true,poisonDuration:8,poisonPoolDamage:2,poisonPunchDamage:3})],
  ['DEMONSKULL',recipe({kind:'demon-skull',capabilities:15})],
  ['DIREFACULTY',recipe({kind:'faculty',bodyColor:[0.2,0.8,0.4,0.75],headColor:[0.9,0.7,0.1,1],female:true,headgear:3,primary:2,secondary:1})],
  ['HEARTMONGER',recipe({kind:'heartmonger',crowCount:3,blindMode:4,summonMode:2})],
]
const cases = []
function add(token, flags, unit, waveOrdinal, authoredRecipe = null, arenaScalars = {}) {
  const random = {baseSpeedUnit:unit, collisionRadiusUnit:unit, randomArmor:unit>=0.5,
    splitManyGateUnit:unit, splitManyUnit:1-unit, splitUnit:unit>=0.5?1:0}
  const options = {flags,random,waveOrdinal,arenaScalars,...(authoredRecipe?{authoredRecipe}:{})}
  const input = {token,flags,random,waveOrdinal,authoredRecipe,arenaScalars}
  try {
    const output = config.evaluateBoneyardEnemyConfig(token,options)
    cases.push({...input, expected:{
      enemyToken:output.enemyToken,flags:output.flags,
      numbers:[output.nativeTypeId,output.attackSpeed,output.baseSpeed,output.chaseSpeed,
        output.collisionRadius,output.experience,output.extraDamage,output.maximumHealth,
        output.primaryDamage,output.scale,output.secondaryDamage,output.tertiaryDamage],
      family:output.family, ignoredSourceFlags:output.ignoredSourceFlags,
      burning:output.burning,flanking:output.flanking,classification:output.classification,
      recipeName:output.recipeName,recipeUid:output.recipeUid,lootPolicies:output.lootPolicies,
      onDeathProgram:output.onDeathProgram,pathfindingMode:output.pathfindingMode,
    }})
  } catch (error) { cases.push({...input, expected:null, error:error.message}) }
}
for (const token of Object.keys(schema.BONEYARD_WAVE_ENEMY_TYPES)) {
  const portal = token === 'PORTAL' ? recipes[0][1] : null
  for (const unit of [0,0.5,1]) add(token,[],unit,0,portal)
  for (const flag of config.BONEYARD_ENEMY_FLAGS) add(token,[flag],0.75,31,portal)
  for (const flags of [
    ['FLAG_HPUP','FLAG_ARMOR','FLAG_SWORD'],['FLAG_SWORD','FLAG_ARMOR','FLAG_HPUP'],
    ['FLAG_SHIELD','FLAG_SHIELDOTHERS','FLAG_SHIELDSTRONG','FLAG_SHIELDFAST'],
    ['FLAG_STRONG','FLAG_CASTFIRE','FLAG_POISONARROW','FLAG_ROTTEN'],
    ['FLAG_HOODED','FLAG_HORNED','FLAG_HELM','FLAG_BURNING','FLAG_SLOW'],
  ]) add(token,flags,0.5,30,portal)
  add(token,['FLAG_SPLITMANY'],1,50,portal,
    {health:1.75,primaryDamage:1.5,secondaryDamage:2,tertiaryDamage:0.5,
      extraDamage:3,chaseSpeed:0.75,attackSpeed:1.25,experience:2})
}
for (const [token,authoredRecipe] of recipes) {
  add(token,[],0.25,44,authoredRecipe)
  add(token,['FLAG_STRONG','FLAG_FAST','FLAG_HPUP'],0.75,45,authoredRecipe)
}
writeFixture('enemy-config-goldens.json','tools/website-oracle/enemy-config-goldens.mjs',
  'Website/frontend/src/game/core-kernels: boneyard-enemy-config.ts, boneyard-enemy-flags.ts, boneyard-enemy-recipes.ts',
  {tokens:Object.entries(schema.BONEYARD_WAVE_ENEMY_TYPES),flags:config.BONEYARD_ENEMY_FLAGS,cases})
console.log(`enemy config: ${cases.length} cases, ${cases.filter(c=>c.expected===null).length} explicit rejections`)
