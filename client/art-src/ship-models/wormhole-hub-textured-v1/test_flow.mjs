import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import { HUB_FLOW_PERIOD, hubFlowMapFragment } from './wormhole-flow.js';

// Evaluate the scalar expressions from the actual GLSL, rather than a second
// implementation, to catch elapsed-time-dependent shear and loop discontinuity.
function expression(name) {
  const source = hubFlowMapFragment.match(new RegExp(`float ${name} = ([^;]+);`))?.[1];
  assert.ok(source, `Missing shader expression: ${name}`);
  const evaluate = new Function('radius', 'uHubTime', 'interior', 'smoothstep',
    `return ${source.replaceAll('sin(', 'Math.sin(')};`);
  const smoothstep = (low, high, value) => {
    const t = Math.max(0, Math.min(1, (value - low) / (high - low)));
    return t * t * (3 - 2 * t);
  };
  return (radius, time = 0, interior = 1) => evaluate(radius, time, interior, smoothstep);
}
const angle = expression('angle'), wave = expression('wave');
const interior = expression('interior');
assert.equal(interior(1), 0, 'The rim must use only the stationary texture');
assert.equal(interior(.98), 0);
assert.equal(interior(.82), 1);
let samples = 0, maximumRadialTwist = 0, maximumAngularGradient = 0;
for (const time of [0, 1, 30, 60, 300, 900, 3600, 21600, 86400]) {
  const rotations = [];
  for (let i = 0; i <= 1000; i++) {
    const radius = i / 1000, turn = angle(radius, time);
    assert.ok(Number.isFinite(turn));
    rotations.push(turn);
    const nextCycle = angle(radius, time + HUB_FLOW_PERIOD);
    assert.ok(Math.abs(Math.sin(turn) - Math.sin(nextCycle)) < 1e-10);
    assert.ok(Math.abs(Math.cos(turn) - Math.cos(nextCycle)) < 1e-10);
    assert.ok(Math.abs(wave(radius, time, 1) - wave(radius, time + HUB_FLOW_PERIOD, 1)) < 1e-10);
    if (i > 0) maximumAngularGradient = Math.max(maximumAngularGradient,
      Math.abs(turn - rotations[i - 1]) * 1000);
    samples++;
  }
  maximumRadialTwist = Math.max(maximumRadialTwist, Math.max(...rotations) - Math.min(...rotations));
}
assert.ok(maximumRadialTwist <= .200001, 'The galaxy must never wind progressively tighter');
assert.ok(maximumAngularGradient <= .800001, 'Radial stretching must stay bounded after hours of animation');
const report = {samples, simulatedHours:24, maximumRadialTwistRadians:maximumRadialTwist,
  maximumAngularGradient, seamlessTimeWrap:true, fixedRim:true,
  method:'Sample the actual GLSL angular and brightness expressions from zero through 24 hours'};
await fs.writeFile(new URL('flow-validation.json', import.meta.url), JSON.stringify(report, null, 2) + '\n');
console.log('PASS:', JSON.stringify(report, null, 2));
