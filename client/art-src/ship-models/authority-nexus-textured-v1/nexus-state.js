import * as THREE from 'three';
import { createWormholeEffect } from './wormhole-effect.js';

/** Share a transform between the operational and attack-disabled exports.
 * Damage locks containment off until the operational variant is restored.
 */
export function createNexusState(operational, disabled, { motion = true } = {}) {
  const group = new THREE.Group();
  group.name = 'Authority Nexus / preview states';
  group.add(operational.scene, disabled.scene);
  const wormhole = operational.scene.getObjectByName('wormhole');
  const field = operational.scene.getObjectByName('containment-field');
  const clip = operational.animations.find(a => a.name === 'WormholeCollapse');
  if (!wormhole || !field || !clip) throw new Error('Operational Nexus is missing its containment assemblies.');
  const effect = createWormholeEffect(wormhole, { playing: motion });
  const mixer = new THREE.AnimationMixer(operational.scene);
  const action = mixer.clipAction(clip);
  action.setLoop(THREE.LoopOnce, 1);
  action.clampWhenFinished = true;
  const cyan = new Map();
  operational.scene.traverse(node => {
    if (node.isMesh && node.material.name.startsWith('Powered cyan ')) {
      cyan.set(node.material, node.material.emissive.clone());
    }
  });
  let attackDisabled = false, operationalPower = 1, traffic = null, disposed = false;
  const effectivePower = () => attackDisabled ? 0 : operationalPower;
  function apply() {
    const power = effectivePower();
    operational.scene.visible = !attackDisabled;
    disabled.scene.visible = attackDisabled;
    action.reset().play(); mixer.setTime((1-power)*clip.duration);
    wormhole.visible = power > .001;
    field.visible = power > .001;
    effect.setPower(power);
    traffic?.setPower(power);
    cyan.forEach((original, material) => material.emissive.copy(original).multiplyScalar(power));
  }
  apply();
  return {
    group,
    get disabled() { return attackDisabled; },
    get power() { return effectivePower(); },
    get active() { return !disposed && (effect.active || !!traffic?.active); },
    setDisabled(value) { attackDisabled = Boolean(value); apply(); },
    setPower(value) {
      if (!attackDisabled && Number.isFinite(value)) operationalPower = THREE.MathUtils.clamp(value, 0, 1);
      apply();
    },
    setMotion(value) { effect.setPlaying(value); },
    attachTraffic(value) {
      traffic = value; group.add(traffic.group); traffic.setPower(effectivePower());
      // A directly opened disabled preview starts with ships already holding.
      if (attackDisabled) {
        const enabled = traffic.group.visible;
        traffic.setEnabled(true);
        for (let frame=0; frame<300 && traffic.active; frame++) traffic.update(.05);
        traffic.setEnabled(enabled);
      }
    },
    update(dt) { if (!disposed) { effect.update(dt); traffic?.update(dt); } },
    dispose() {
      if (disposed) return;
      disposed = true; effect.dispose(); traffic?.dispose(); mixer.stopAllAction();
    },
  };
}
