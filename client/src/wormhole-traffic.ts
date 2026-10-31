// Decorative traffic for the Wormhole Hub: nine freighters curving in over the
// ring's face and down into the portal. Ported from
// art-src/ship-models/wormhole-hub-textured-v1 (wormhole-traffic.js): the
// routes, timing and scale are the asset's validated ones. Ships are instanced
// LOD2 hulls in the station's own units (ring in XY, 17.1 across, passage along
// Z), so the group belongs under the station's local frame. Never fleets: no
// markers, picks or labels.
import * as THREE from "three";
import { mergeGeometries } from "three/addons/utils/BufferGeometryUtils.js";

export type TrafficShip = "tiny-freighter" | "small-freighter" | "medium-freighter";

const FLIGHT_SECONDS = 26, CYCLE_SECONDS = 36, CONSUME_START = .92, ENTRY_PROGRESS = .969;
/** Hull lengths in station units: about 1.5–2.3% of the station's width. */
export const TRAFFIC_LENGTHS: Readonly<Record<TrafficShip, number>> = {
  "tiny-freighter": .25, "small-freighter": .32, "medium-freighter": .40,
};
const FLEET: TrafficShip[] = ["tiny-freighter", "small-freighter", "medium-freighter",
  "small-freighter", "tiny-freighter", "medium-freighter",
  "tiny-freighter", "medium-freighter", "small-freighter"];
/** The membrane's curve (the asset's portal-profile.json): its anchored rim and
 * how far its centre sinks behind it. */
const PORTAL_RADIUS = 3.685, PORTAL_DEPTH = 1.6;
const UP = new THREE.Vector3(0, 1, 0), ORIGIN = new THREE.Vector3();
const SCALE = new THREE.Vector3(), TAIL = new THREE.Vector3();
const LOOK = new THREE.Matrix4(), MATRIX = new THREE.Matrix4();
const HIDDEN = new THREE.Matrix4().makeScale(0, 0, 0);
const COLOR = new THREE.Color();

/** The ring lies in XY. Approach from +Z and clear the full 1.73-unit hardware
 * depth before descending to the energy surface inside the 7.21-unit opening. */
export function trafficRoute(progress: number, angle: number, target = new THREE.Vector3()): THREE.Vector3 {
  const p = THREE.MathUtils.clamp(progress, 0, 1);
  const r = 8.4 * Math.pow(1 - p, 1.1) + .18;
  const a = angle + .20 * Math.sin(p * Math.PI);
  return target.set(Math.cos(a) * r, Math.sin(a) * r, portalDepth(r) + 4.2 * Math.pow(1 - p, .78) - .24);
}

/** The membrane's depth (−Z) at a radius from the passage's axis. */
export function portalDepth(radius: number): number {
  return -PORTAL_DEPTH * (1 - Math.min(1, radius / PORTAL_RADIUS) ** 2);
}

/** Where a hull fades in: its route starts beyond the rim at zero size. */
export const TRAFFIC_VISIBLE_FROM = .035;

/** One instanced mesh per material of a hull, normalized to unit length. */
function shipBatches(template: THREE.Object3D, capacity: number, parent: THREE.Group): THREE.InstancedMesh[] {
  template.updateMatrixWorld(true);
  const bounds = new THREE.Box3().setFromObject(template);
  const size = bounds.getSize(new THREE.Vector3());
  const center = bounds.getCenter(new THREE.Vector3());
  if (!Number.isFinite(size.z) || size.z <= 0) throw new Error("Traffic ship has no measurable hull.");
  const normalize = new THREE.Matrix4().makeScale(1 / size.z, 1 / size.z, 1 / size.z)
    .multiply(new THREE.Matrix4().makeTranslation(-center.x, -center.y, -center.z));
  const byMaterial = new Map<THREE.Material, THREE.BufferGeometry[]>();
  template.traverse(node => {
    const mesh = node as THREE.Mesh;
    if (!mesh.isMesh) return;
    if ((mesh as THREE.SkinnedMesh).isSkinnedMesh || Array.isArray(mesh.material)) throw new Error("Traffic expects the approved static LOD2 ship meshes.");
    const geometry = mesh.geometry.clone().applyMatrix4(new THREE.Matrix4().multiplyMatrices(normalize, mesh.matrixWorld));
    if (!byMaterial.has(mesh.material)) byMaterial.set(mesh.material, []);
    byMaterial.get(mesh.material)!.push(geometry);
  });
  const batches: THREE.InstancedMesh[] = [];
  for (const [source, geometries] of byMaterial) {
    const geometry = mergeGeometries(geometries);
    geometries.forEach(g => g.dispose());
    if (!geometry) throw new Error("Could not consolidate the traffic ship.");
    const mesh = new THREE.InstancedMesh(geometry, source.clone(), capacity);
    mesh.name = `Traffic / ${template.name} / ${source.name}`;
    mesh.instanceMatrix.setUsage(THREE.DynamicDrawUsage);
    mesh.frustumCulled = false;
    parent.add(mesh);
    batches.push(mesh);
  }
  return batches;
}

interface Ship {
  id: number; type: TrafficShip; slot: number; length: number; angle: number; hold: number; clock: number;
  position: THREE.Vector3; tangent: THREE.Vector3; quaternion: THREE.Quaternion; entry: THREE.Vector3;
  entryQuaternion: THREE.Quaternion; visible: boolean; renderedLength: number;
}

export interface WormholeTraffic {
  readonly group: THREE.Group;
  /** False while hidden or disposed: no frames are needed for traffic. */
  readonly active: boolean;
  readonly stats: { ships: number; shipCounts: Partial<Record<TrafficShip, number>>; maximumTriangles: number };
  snapshot(): { id: number; type: TrafficShip; visible: boolean; renderedLength: number; position: [number, number, number] }[];
  setEnabled(enabled: boolean): void;
  /** Advance the stream; a long gap counts as one short step. */
  update(deltaSeconds: number): void;
  /** Frees merged geometry and cloned materials; the templates stay alive. */
  dispose(): void;
}

/** Instanced traffic built from loaded hulls (any scale or centring). The
 * Market Hub's containment is always powered, so ships never divert to hold.
 * `scale` enlarges hulls, wakes and ripples together from the asset's study. */
export function createWormholeTraffic(templates: Record<TrafficShip, THREE.Object3D>, { enabled = true, scale: size = 1 } = {}): WormholeTraffic {
  for (const type of Object.keys(TRAFFIC_LENGTHS) as TrafficShip[]) {
    if (!templates[type]) throw new Error(`Missing traffic ship template: ${type}`);
  }
  const group = new THREE.Group();
  group.name = "Small wormhole traffic";
  const capacities: Partial<Record<TrafficShip, number>> = {};
  const ships: Ship[] = FLEET.map((type, i) => {
    const tier = Math.floor(i / 4);
    const slot = capacities[type] ?? 0;
    capacities[type] = slot + 1;
    return {
      id: i, type, slot, length: TRAFFIC_LENGTHS[type] * size,
      angle: Math.PI / 4 + (i % 4) * Math.PI / 2 + tier * .16,
      hold: .52 - tier * .06, clock: (3 + i * CYCLE_SECONDS / FLEET.length) % CYCLE_SECONDS,
      position: new THREE.Vector3(), tangent: new THREE.Vector3(), quaternion: new THREE.Quaternion(),
      entry: new THREE.Vector3(), entryQuaternion: new THREE.Quaternion(), visible: false, renderedLength: 0,
    };
  });
  const batches = Object.fromEntries((Object.entries(capacities) as [TrafficShip, number][]).map(([type, count]) =>
    [type, shipBatches(templates[type], count, group)])) as Record<TrafficShip, THREE.InstancedMesh[]>;
  const plumeGeometry = new THREE.ConeGeometry(1, 1, 5, 1, true).rotateX(Math.PI / 2).translate(0, 0, .5);
  const colors = new Float32Array(plumeGeometry.attributes.position.count * 3);
  for (let i = 0; i < colors.length / 3; i++) {
    const intensity = Math.pow(1 - THREE.MathUtils.clamp(plumeGeometry.attributes.position.getZ(i), 0, 1), 1.25);
    colors.set([intensity, intensity, intensity], i * 3);
  }
  plumeGeometry.setAttribute("color", new THREE.BufferAttribute(colors, 3));
  const plumes = new THREE.InstancedMesh(plumeGeometry, new THREE.MeshBasicMaterial({
    color: 0x89ddff, vertexColors: true, transparent: true, opacity: .66,
    blending: THREE.AdditiveBlending, depthWrite: false, side: THREE.DoubleSide, toneMapped: false,
  }), ships.length);
  plumes.name = "Traffic / small engine wakes";
  plumes.frustumCulled = false;
  const rings = new THREE.InstancedMesh(new THREE.RingGeometry(.85, 1, 32),
    new THREE.MeshBasicMaterial({ color: 0xa2b9ff, transparent: true, opacity: .45,
      blending: THREE.AdditiveBlending, depthWrite: false, side: THREE.DoubleSide, toneMapped: false }), ships.length);
  rings.name = "Traffic / entry ripples";
  rings.frustumCulled = false;
  plumes.instanceMatrix.setUsage(THREE.DynamicDrawUsage);
  rings.instanceMatrix.setUsage(THREE.DynamicDrawUsage);
  group.add(plumes, rings);
  // Each entry ripple lies on the curved membrane, turned to its slope.
  const slope = 2 * PORTAL_DEPTH / (PORTAL_RADIUS * PORTAL_RADIUS);
  for (const ship of ships) {
    trafficRoute(ENTRY_PROGRESS, ship.angle, ship.entry);
    ship.entry.z = portalDepth(Math.hypot(ship.entry.x, ship.entry.y)) + .028;
    ship.entryQuaternion.setFromUnitVectors(new THREE.Vector3(0, 0, 1),
      new THREE.Vector3(-slope * ship.entry.x, -slope * ship.entry.y, 1).normalize());
  }
  let shown = enabled, disposed = false;
  group.visible = shown;
  const scratch = new THREE.Vector3();

  function pose(ship: Ship): void {
    const p = Math.min(ship.clock / FLIGHT_SECONDS, 1);
    const entryScale = 1 - THREE.MathUtils.smoothstep(p, CONSUME_START, ENTRY_PROGRESS);
    trafficRoute(p, ship.angle, ship.position);
    trafficRoute(Math.min(1, p + .001), ship.angle, ship.tangent);
    trafficRoute(Math.max(0, p - .001), ship.angle, scratch);
    ship.tangent.sub(scratch).normalize();
    LOOK.lookAt(ORIGIN, ship.tangent, UP);
    ship.quaternion.setFromRotationMatrix(LOOK);
    // The last short contraction consumes the ship at the event horizon, so it
    // never reappears behind the membrane when viewed from the back.
    ship.visible = p < ENTRY_PROGRESS;
    const scale = ship.visible ? ship.length * THREE.MathUtils.smoothstep(p, 0, TRAFFIC_VISIBLE_FROM) * entryScale : 0;
    ship.renderedLength = scale;
    MATRIX.compose(ship.position, ship.quaternion, SCALE.setScalar(scale));
    for (const batch of batches[ship.type]) batch.setMatrixAt(ship.slot, MATRIX);
    if (scale > 0) {
      TAIL.set(0, 0, scale * .48).applyQuaternion(ship.quaternion).add(ship.position);
      MATRIX.compose(TAIL, ship.quaternion, SCALE.set(scale * .045, scale * .045, scale * .75));
      plumes.setMatrixAt(ship.id, MATRIX);
      plumes.setColorAt(ship.id, COLOR.setScalar(1));
    } else plumes.setMatrixAt(ship.id, HIDDEN);
    const arrival = (ship.clock - FLIGHT_SECONDS * ENTRY_PROGRESS) / .85;
    if (arrival > 0 && arrival < 1) {
      MATRIX.compose(ship.entry, ship.entryQuaternion, SCALE.setScalar((.06 + .30 * arrival) * size));
      rings.setMatrixAt(ship.id, MATRIX);
      rings.setColorAt(ship.id, COLOR.setScalar(Math.pow(1 - arrival, 2)));
    } else rings.setMatrixAt(ship.id, HIDDEN);
  }
  function refresh(): void {
    ships.forEach(pose);
    for (const mesh of group.children as THREE.InstancedMesh[]) mesh.instanceMatrix.needsUpdate = true;
    if (plumes.instanceColor) plumes.instanceColor.needsUpdate = true;
    if (rings.instanceColor) rings.instanceColor.needsUpdate = true;
  }
  refresh();
  return {
    group,
    get active() { return !disposed && shown; },
    get stats() {
      return { ships: ships.length, shipCounts: { ...capacities },
        maximumTriangles: (group.children as THREE.InstancedMesh[]).reduce((sum, mesh) =>
          sum + (mesh.geometry.index?.count ?? mesh.geometry.attributes.position.count) / 3 * mesh.count, 0) };
    },
    snapshot() {
      return ships.map(ship => ({ id: ship.id, type: ship.type, visible: shown && ship.visible,
        renderedLength: ship.renderedLength, position: ship.position.toArray() as [number, number, number] }));
    },
    setEnabled(value) { shown = value; group.visible = value; },
    update(deltaSeconds) {
      if (!this.active || !Number.isFinite(deltaSeconds) || deltaSeconds <= 0) return;
      const dt = Math.min(deltaSeconds, .05);
      for (const ship of ships) ship.clock = (ship.clock + dt) % CYCLE_SECONDS;
      refresh();
    },
    dispose() {
      if (disposed) return;
      disposed = true;
      group.removeFromParent();
      for (const mesh of group.children as THREE.InstancedMesh[]) {
        mesh.geometry.dispose();
        (mesh.material as THREE.Material).dispose();
        mesh.dispose();
      }
      group.clear();
    },
  };
}
