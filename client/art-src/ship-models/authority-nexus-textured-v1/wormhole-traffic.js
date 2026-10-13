import * as THREE from 'three';
import { mergeGeometries } from 'three/addons/utils/BufferGeometryUtils.js';

const FLIGHT_SECONDS = 26, CYCLE_SECONDS = 36, ENTRY_PROGRESS = .969;
const SHIP_LENGTHS = {interceptor:.17, 'tiny-freighter':.25, 'small-freighter':.30, 'medium-freighter':.36};
const FLEET = ['tiny-freighter', 'interceptor', 'small-freighter', 'tiny-freighter', 'medium-freighter',
  'interceptor', 'small-freighter', 'tiny-freighter', 'medium-freighter', 'interceptor'];
const UP = new THREE.Vector3(0, 1, 0), ORIGIN = new THREE.Vector3();
const SCALE = new THREE.Vector3(), TAIL = new THREE.Vector3();
const LOOK = new THREE.Matrix4(), MATRIX = new THREE.Matrix4();
const HIDDEN = new THREE.Matrix4().makeScale(0, 0, 0);
const COLOR = new THREE.Color();

// Paths enter between the four stabilizers. The descent only reaches the
// membrane inside the opening, after clearing the raised containment collar.
export function trafficRoute(progress, angle, target = new THREE.Vector3()) {
  const p = THREE.MathUtils.clamp(progress, 0, 1);
  const r = 11.2 * Math.pow(1 - p, 1.12) + .10;
  const a = angle + .20 * Math.sin(p * Math.PI);
  return target.set(Math.cos(a) * r, 3.35 - 2.20 * p - 1.94 * Math.pow(p, 5), Math.sin(a) * r);
}

function portalHeight(radius) {
  const profile = [[0,-.47],[.25,-.44],[.8,-.35],[1.55,-.2],[2.3,.08],[3.18,.37],[4,.52]];
  for (let i=1;i<profile.length;i++) {
    if (radius <= profile[i][0]) {
      const [a,h0] = profile[i-1], [b,h1] = profile[i];
      return THREE.MathUtils.lerp(h0,h1,(radius-a)/(b-a));
    }
  }
  return .52;
}

function shipBatches(template, capacity, parent) {
  template.updateMatrixWorld(true);
  const bounds = new THREE.Box3().setFromObject(template);
  const size = bounds.getSize(new THREE.Vector3());
  const center = bounds.getCenter(new THREE.Vector3());
  if (!Number.isFinite(size.z) || size.z <= 0) throw new Error('Traffic ship has no measurable hull.');
  const normalize = new THREE.Matrix4().makeScale(1/size.z,1/size.z,1/size.z)
    .multiply(new THREE.Matrix4().makeTranslation(-center.x,-center.y,-center.z));
  const byMaterial = new Map();
  template.traverse(node => {
    if (!node.isMesh) return;
    if (node.isSkinnedMesh || Array.isArray(node.material)) throw new Error('Traffic expects the approved static LOD2 ship meshes.');
    const geometry = node.geometry.clone().applyMatrix4(new THREE.Matrix4().multiplyMatrices(normalize,node.matrixWorld));
    if (!byMaterial.has(node.material)) byMaterial.set(node.material, []);
    byMaterial.get(node.material).push(geometry);
  });
  const batches = [];
  for (const [source, geometries] of byMaterial) {
    const geometry = mergeGeometries(geometries);
    geometries.forEach(g => g.dispose());
    if (!geometry) throw new Error('Could not consolidate the traffic ship.');
    const mesh = new THREE.InstancedMesh(geometry,source.clone(),capacity);
    mesh.name = 'Traffic / '+template.name+' / '+source.name;
    mesh.instanceMatrix.setUsage(THREE.DynamicDrawUsage);
    mesh.frustumCulled = false;
    parent.add(mesh); batches.push(mesh);
  }
  return batches;
}

/** Decorative, instanced traffic for the Authority Nexus viewer. Ships use
 * Y-up/-Z-forward GLBs and remain separate from the exported station model.
 */
export function createWormholeTraffic(templates, { enabled = true } = {}) {
  for (const type of Object.keys(SHIP_LENGTHS)) {
    if (!templates[type]) throw new Error('Missing traffic ship template: '+type);
  }
  const group = new THREE.Group(); group.name = 'Small wormhole traffic';
  const capacities = {};
  const ships = FLEET.map((type,i) => {
    const tier = Math.floor(i/4);
    const slot = capacities[type] ?? 0;
    capacities[type] = slot+1;
    return {
      id:i, type, slot, length:SHIP_LENGTHS[type],
      angle:Math.PI/4+(i%4)*Math.PI/2+tier*.16,
      hold:.52-tier*.06, clock:(3+i*CYCLE_SECONDS/FLEET.length)%CYCLE_SECONDS, diversion:null,
      position:new THREE.Vector3(), tangent:new THREE.Vector3(), quaternion:new THREE.Quaternion(),
      entry:new THREE.Vector3(), visible:false,
    };
  });
  const batches = Object.fromEntries(Object.entries(capacities).map(([type,count]) =>
    [type,shipBatches(templates[type],count,group)]));
  const plumeGeometry = new THREE.ConeGeometry(1,1,5,1,true).rotateX(Math.PI/2).translate(0,0,.5);
  const colors = new Float32Array(plumeGeometry.attributes.position.count*3);
  for (let i=0;i<colors.length/3;i++) {
    const intensity = Math.pow(1-THREE.MathUtils.clamp(plumeGeometry.attributes.position.getZ(i),0,1),1.25);
    colors.set([intensity,intensity,intensity],i*3);
  }
  plumeGeometry.setAttribute('color',new THREE.BufferAttribute(colors,3));
  const plumes = new THREE.InstancedMesh(plumeGeometry,new THREE.MeshBasicMaterial({
    color:0x89ddff, vertexColors:true, transparent:true, opacity:.66,
    blending:THREE.AdditiveBlending, depthWrite:false, side:THREE.DoubleSide, toneMapped:false,
  }),ships.length);
  plumes.name = 'Traffic / small engine wakes'; plumes.frustumCulled = false;
  const rings = new THREE.InstancedMesh(new THREE.RingGeometry(.85,1,32).rotateX(-Math.PI/2),
    new THREE.MeshBasicMaterial({color:0xa2b9ff,transparent:true,opacity:.45,
      blending:THREE.AdditiveBlending,depthWrite:false,side:THREE.DoubleSide,toneMapped:false}),ships.length);
  rings.name = 'Traffic / entry ripples'; rings.frustumCulled = false;
  plumes.instanceMatrix.setUsage(THREE.DynamicDrawUsage);
  rings.instanceMatrix.setUsage(THREE.DynamicDrawUsage);
  group.add(plumes,rings);
  for (const ship of ships) {
    trafficRoute(ENTRY_PROGRESS,ship.angle,ship.entry);
    ship.entry.y = portalHeight(Math.hypot(ship.entry.x,ship.entry.z))+.028;
  }
  let power = 1, shown = Boolean(enabled), disposed = false;
  group.visible = shown;
  const scratch = new THREE.Vector3();

  function pose(ship) {
    const p = Math.min(ship.clock/FLIGHT_SECONDS,1);
    let entryScale = 1-THREE.MathUtils.smoothstep(p,.94,ENTRY_PROGRESS);
    if (ship.diversion) {
      const t = Math.min(1,ship.diversion.elapsed/ship.diversion.duration);
      ship.diversion.curve.getPoint(t,ship.position);
      ship.diversion.curve.getTangent(t,ship.tangent);
      entryScale = THREE.MathUtils.lerp(ship.diversion.startScale,1,THREE.MathUtils.smoothstep(t,0,.6));
    } else {
      trafficRoute(p,ship.angle,ship.position);
      trafficRoute(Math.min(1,p+.001),ship.angle,ship.tangent);
      trafficRoute(Math.max(0,p-.001),ship.angle,scratch);
      ship.tangent.sub(scratch).normalize();
    }
    LOOK.lookAt(ORIGIN,ship.tangent,UP);
    ship.quaternion.setFromRotationMatrix(LOOK);
    // The last short contraction consumes the ship at the event horizon, so it
    // never reappears underneath the membrane when viewed from below.
    ship.visible = Boolean(ship.diversion) || p < ENTRY_PROGRESS;
    const scale = ship.visible ? ship.length*THREE.MathUtils.smoothstep(p,0,.035)*entryScale : 0;
    ship.renderedLength = scale;
    MATRIX.compose(ship.position,ship.quaternion,SCALE.setScalar(scale));
    for (const batch of batches[ship.type]) batch.setMatrixAt(ship.slot,MATRIX);
    if (scale > 0) {
      TAIL.set(0,0,scale*.48).applyQuaternion(ship.quaternion).add(ship.position);
      const cruising = power > .94 || ship.diversion || ship.clock < ship.hold*FLIGHT_SECONDS-.001;
      const wake = cruising ? .75 : .20;
      MATRIX.compose(TAIL,ship.quaternion,SCALE.set(scale*.045,scale*.045,scale*wake));
      plumes.setMatrixAt(ship.id,MATRIX);
      plumes.setColorAt(ship.id,COLOR.setScalar(cruising ? 1 : .35));
    } else plumes.setMatrixAt(ship.id,HIDDEN);

    const arrival = (ship.clock-FLIGHT_SECONDS*ENTRY_PROGRESS)/.85;
    if (power > .94 && !ship.diversion && arrival > 0 && arrival < 1) {
      MATRIX.compose(ship.entry,new THREE.Quaternion(),SCALE.setScalar(.06+.30*arrival));
      rings.setMatrixAt(ship.id,MATRIX);
      rings.setColorAt(ship.id,COLOR.setScalar(Math.pow(1-arrival,2)));
    } else rings.setMatrixAt(ship.id,HIDDEN);
  }
  function refresh() {
    ships.forEach(pose);
    group.children.forEach(mesh => { mesh.instanceMatrix.needsUpdate = true; });
    if (plumes.instanceColor) plumes.instanceColor.needsUpdate = true;
    if (rings.instanceColor) rings.instanceColor.needsUpdate = true;
  }
  refresh();
  return {
    group,
    get active() {
      return !disposed && shown && (power > .94 || ships.some(s => s.diversion || s.clock < s.hold*FLIGHT_SECONDS-.0001));
    },
    get stats() {
      return {ships:ships.length, shipCounts:{...capacities}, shipLengths:Object.values(SHIP_LENGTHS), stationDiameter:19.8,
        arrivalIntervalSeconds:CYCLE_SECONDS/ships.length,
        renderSections:group.children.length,
        maximumTriangles:group.children.reduce((sum,m) => sum+(m.geometry.index?.count ?? m.geometry.attributes.position.count)/3*m.count,0)};
    },
    snapshot() {
      return ships.map(s => ({id:s.id,type:s.type,length:s.length,renderedLength:s.renderedLength,clock:s.clock,approachAngle:s.angle,
        visible:shown && s.visible,position:s.position.toArray(),quaternion:s.quaternion.toArray(),
        state:s.diversion ? 'diverting' : s.clock >= FLIGHT_SECONDS ? 'between arrivals' :
          power <= .94 && s.clock >= s.hold*FLIGHT_SECONDS-.0001 ? 'holding' : 'approaching'}));
    },
    setEnabled(value) { shown = Boolean(value); group.visible = shown; },
    setPower(value) {
      if (!Number.isFinite(value)) return;
      const previous = power; power = THREE.MathUtils.clamp(value,0,1);
      if (previous > .94 && power <= .94) {
        for (const ship of ships) {
          const p = ship.clock/FLIGHT_SECONDS;
          if (p <= ship.hold || p >= 1 || ship.diversion) continue;
          if (p >= ENTRY_PROGRESS) { ship.clock = FLIGHT_SECONDS; continue; }
          const start = ship.position.clone(), end = trafficRoute(ship.hold,ship.angle);
          // Lift clear of the membrane before returning to a holding point.
          const a = start.clone().add(new THREE.Vector3(0,1.3,0));
          const b = end.clone().add(new THREE.Vector3(0,.8,0));
          ship.diversion = {curve:new THREE.CubicBezierCurve3(start,a,b,end),elapsed:0,
            startScale:1-THREE.MathUtils.smoothstep(p,.94,ENTRY_PROGRESS),
            duration:2.5+start.distanceTo(end)*.30};
        }
      }
      refresh();
    },
    update(deltaSeconds) {
      if (!this.active || !Number.isFinite(deltaSeconds) || deltaSeconds <= 0) return;
      const dt = Math.min(deltaSeconds,.05);
      for (const ship of ships) {
        if (ship.diversion) {
          ship.diversion.elapsed += dt;
          if (ship.diversion.elapsed >= ship.diversion.duration) {
            ship.diversion = null; ship.clock = ship.hold*FLIGHT_SECONDS;
          }
        } else if (power > .94) ship.clock = (ship.clock+dt)%CYCLE_SECONDS;
        else if (ship.clock < ship.hold*FLIGHT_SECONDS) ship.clock = Math.min(ship.hold*FLIGHT_SECONDS,ship.clock+dt);
      }
      refresh();
    },
    dispose() {
      if (disposed) return;
      disposed = true;
      group.removeFromParent();
      for (const mesh of group.children) { mesh.geometry.dispose(); mesh.material.dispose(); mesh.dispose(); }
      group.clear();
    },
  };
}
