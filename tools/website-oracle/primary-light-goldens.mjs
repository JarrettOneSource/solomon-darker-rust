import { loadKernel, writeFixture } from './oracle.mjs'
const light = await loadKernel('core-kernels/native-primary-light-sources.ts')
const cases = []
for (const id of [1, 731, 4294967297]) {
  for (const frame of [0, 1, 17, 36, 37, 100]) {
    const position = { x: 100.25, y: -75.125 }
    const source = { ageTicks: frame, id, life: frame / 37, lightGlow: frame / 17, origin: position, position }
    for (const multipleShadows of [false, true]) {
      const pointGain = frame % 2 ? -0.5 : 0.7
      cases.push({ id, frame, position, pointGain, multipleShadows, life: source.life, lightGlow: source.lightGlow,
        expected: [light.nativeFireExplosionLightSource(source, pointGain, multipleShadows),
          light.nativeFireEmberLightSource(source, frame), light.nativeFireImpactLightSource(source),
          light.nativeFireGoodImpLightSource(source, frame), light.nativeFireballLightSource(source, frame, multipleShadows),
          light.etherPrimaryImpactLightSource(source)] })
    }
  }
}
writeFixture('primary-light-goldens.json', 'tools/website-oracle/primary-light-goldens.mjs',
  'Website/frontend/src/game/core-kernels/native-primary-light-sources.ts', { cases })
