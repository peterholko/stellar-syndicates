import * as THREE from "three";
import { mergeGeometries } from "three/addons/utils/BufferGeometryUtils.js";
import { BATTLE_MODEL_MAX_SHIPS, BATTLE_MODEL_TILT, type BattleModelShip, type BattleWreckPiece } from "./battle-model-policy";

/** Opacity belongs to a represented hull, not to its shared material. */
export function instanceOpacity(material: THREE.Material): void {
  material.transparent = true;
  material.onBeforeCompile = shader => {
    shader.vertexShader = shader.vertexShader.replace("#include <common>",
      "#include <common>\nattribute float battleOpacity;\nvarying float vBattleOpacity;")
      .replace("#include <begin_vertex>", "#include <begin_vertex>\nvBattleOpacity = battleOpacity;");
    shader.fragmentShader = shader.fragmentShader.replace("#include <common>",
      "#include <common>\nvarying float vBattleOpacity;")
      .replace("#include <color_fragment>", "#include <color_fragment>\ndiffuseColor.a *= vBattleOpacity;");
  };
  material.customProgramCacheKey = () => "battle-instance-opacity-v1";
}

/** Wreck sections: per-instance opacity plus a char that darkens the hull's
 * colour and puts out its lights as it burns cold. */
function instanceWreck(material: THREE.Material): void {
  material.transparent = true;
  material.onBeforeCompile = shader => {
    shader.vertexShader = shader.vertexShader.replace("#include <common>",
      "#include <common>\nattribute float battleOpacity;\nattribute float battleChar;\nvarying float vBattleOpacity;\nvarying float vBattleChar;")
      .replace("#include <begin_vertex>", "#include <begin_vertex>\nvBattleOpacity = battleOpacity;\nvBattleChar = battleChar;");
    shader.fragmentShader = shader.fragmentShader.replace("#include <common>",
      "#include <common>\nvarying float vBattleOpacity;\nvarying float vBattleChar;")
      .replace("#include <color_fragment>", "#include <color_fragment>\ndiffuseColor.a *= vBattleOpacity;\ndiffuseColor.rgb *= 1.0 - 0.75 * vBattleChar;")
      .replace("#include <emissivemap_fragment>", "#include <emissivemap_fragment>\ntotalEmissiveRadiance *= 1.0 - vBattleChar;");
  };
  material.customProgramCacheKey = () => "battle-wreck-v1";
}

/** Bake static node transforms, then consolidate compatible sections sharing a
 * material. UVs, normals, material maps and every LOD2 triangle are retained.
 * These approved hulls have no skeletal animation or independently aimed guns. */
export function mergeBattleHull(source: THREE.Group): { geometry: THREE.BufferGeometry; material: THREE.Material }[] {
  source.updateMatrixWorld(true);
  const groups = new Map<string, { parts: THREE.BufferGeometry[]; material: THREE.Material }>();
  source.traverse(object => {
    if (!(object instanceof THREE.Mesh)) return;
    if (object instanceof THREE.SkinnedMesh) throw new Error("Battle hull requires a static mesh");
    const original: THREE.BufferGeometry = object.geometry;
    const materials = Array.isArray(object.material) ? object.material : [object.material];
    const sections = Array.isArray(object.material) ? original.groups : [{ start: 0,
      count: original.index?.count ?? original.getAttribute("position").count, materialIndex: 0 }];
    for (const section of sections) {
      const material = materials[section.materialIndex ?? 0];
      const geometry = original.clone().applyMatrix4(object.matrixWorld);
      const indices = original.index?.array ?? Array.from({ length: geometry.getAttribute("position").count }, (_, i) => i);
      const selected = Array.from(indices.slice(section.start, section.start + section.count));
      // Baking a mirrored node also bakes its handedness; reverse the winding
      // that Three previously corrected using the node's negative determinant.
      if (object.matrixWorld.determinant() < 0) for (let i = 0; i < selected.length; i += 3) {
        [selected[i], selected[i + 2]] = [selected[i + 2], selected[i]];
      }
      geometry.setIndex(selected); geometry.clearGroups();
      const signature = Object.entries(geometry.attributes).sort(([a], [b]) => a.localeCompare(b))
        .map(([name, attr]) => `${name}:${attr.itemSize}:${attr.normalized}:${attr.array.constructor.name}`).join("|");
      const key = `${material.uuid}:${signature}`;
      let group = groups.get(key);
      if (!group) { group = { parts: [], material }; groups.set(key, group); }
      group.parts.push(geometry);
    }
  });
  const merged: { geometry: THREE.BufferGeometry; material: THREE.Material }[] = [];
  try {
    for (const { parts, material } of groups.values()) {
      const geometry = mergeGeometries(parts, false);
      if (!geometry) throw new Error("Incompatible static hull sections");
      merged.push({ geometry, material });
    }
  } catch (error) {
    for (const part of merged) part.geometry.dispose();
    throw error;
  } finally {
    for (const { parts } of groups.values()) for (const part of parts) part.dispose();
  }
  return merged;
}

const pitch = new THREE.Matrix4();
const yaw = new THREE.Matrix4(), scale = new THREE.Matrix4();

/** Camera sits on +Y looking down, up=-Z. Each hull is pitched by the
 * battlefield's inclination; compensate that pitch so the projected nose stays
 * exactly on the served (screen) heading, including diagonals and turns. On a
 * tilted battlefield, depthSlope brings lower hulls nearer the camera. */
export function battleHullMatrix(ship: BattleModelShip, width: number, height: number, out: THREE.Matrix4,
  tilt = BATTLE_MODEL_TILT, depthSlope = 0): void {
  const angle = Math.atan2(-Math.cos(ship.angle), -Math.sin(ship.angle) / Math.cos(tilt));
  pitch.makeRotationX(tilt);
  yaw.makeRotationY(angle); scale.makeScale(ship.canvasPx / 2.24, ship.canvasPx / 2.24, ship.canvasPx / 2.24);
  out.copy(pitch).multiply(yaw).multiply(scale);
  out.setPosition(ship.x - width / 2, (ship.y - height / 2) * depthSlope, ship.y - height / 2);
}

export class BattleInstancedHull {
  readonly group = new THREE.Group();
  readonly meshes: THREE.InstancedMesh[] = [];
  private opacity = new THREE.InstancedBufferAttribute(new Float32Array(BATTLE_MODEL_MAX_SHIPS), 1);
  private matrix = new THREE.Matrix4();

  constructor(source: THREE.Group) {
    this.opacity.setUsage(THREE.DynamicDrawUsage);
    for (const part of mergeBattleHull(source)) {
      part.geometry.setAttribute("battleOpacity", this.opacity);
      const material = part.material.clone();
      instanceOpacity(material);
      const mesh = new THREE.InstancedMesh(part.geometry, material, BATTLE_MODEL_MAX_SHIPS);
      if (this.meshes.length) mesh.instanceMatrix = this.meshes[0].instanceMatrix;
      else mesh.instanceMatrix.setUsage(THREE.DynamicDrawUsage);
      mesh.frustumCulled = false; // admission already culls in screen space
      mesh.count = 0;
      this.meshes.push(mesh); this.group.add(mesh);
    }
  }

  update(ships: readonly BattleModelShip[], width: number, height: number, tilt = BATTLE_MODEL_TILT,
    depthSlope = 0): void {
    const count = Math.min(ships.length, BATTLE_MODEL_MAX_SHIPS);
    this.group.visible = count > 0;
    if (!count || !this.meshes.length) return;
    for (let i = 0; i < count; i++) {
      battleHullMatrix(ships[i], width, height, this.matrix, tilt, depthSlope);
      this.meshes[0].setMatrixAt(i, this.matrix);
      this.opacity.setX(i, ships[i].alpha);
    }
    this.opacity.needsUpdate = true;
    this.meshes[0].instanceMatrix.needsUpdate = true;
    for (const mesh of this.meshes) mesh.count = count;
  }

  dispose(): void {
    this.group.removeFromParent();
    for (const mesh of this.meshes) {
      mesh.dispose(); mesh.geometry.dispose(); (mesh.material as THREE.Material).dispose();
    }
  }
}

export const BATTLE_WRECK_INSTANCES = 16;

/** A hull's wreck sections: the intact hull's triangles partitioned by where
 * their centroids fall along it (nose −z … tail +z) at the per-type cuts, drawn
 * as separately posed instances. Sections share the intact hull's vertex data;
 * every triangle belongs to exactly one section. */
export class BattleHullChunks {
  readonly group = new THREE.Group();
  readonly chunks: { meshes: THREE.InstancedMesh[]; pivot: THREE.Vector3; triangles: number;
    opacity: THREE.InstancedBufferAttribute; char: THREE.InstancedBufferAttribute }[] = [];
  private matrix = new THREE.Matrix4();
  private local = new THREE.Matrix4();
  private turn = new THREE.Matrix4();
  private euler = new THREE.Euler(0, 0, 0, "YXZ");

  constructor(hull: BattleInstancedHull, cuts: readonly number[]) {
    const n = cuts.length + 1;
    const sums = Array.from({ length: n }, () => [0, 0, 0, 0]);
    for (let c = 0; c < n; c++) {
      const attr = () => new THREE.InstancedBufferAttribute(new Float32Array(BATTLE_WRECK_INSTANCES), 1).setUsage(THREE.DynamicDrawUsage);
      this.chunks.push({ meshes: [], pivot: new THREE.Vector3(), triangles: 0, opacity: attr(), char: attr() });
    }
    for (const mesh of hull.meshes) {
      const source = mesh.geometry, pos = source.getAttribute("position"), index = source.index!.array;
      const subsets: number[][] = Array.from({ length: n }, () => []);
      for (let t = 0; t < index.length; t += 3) {
        const a = index[t], b = index[t + 1], c = index[t + 2];
        const cz = (pos.getZ(a) + pos.getZ(b) + pos.getZ(c)) / 3;
        let chunk = 0;
        while (chunk < cuts.length && (cz + 1) / 2 > cuts[chunk]) chunk++;
        subsets[chunk].push(a, b, c);
        const sum = sums[chunk];
        sum[0] += (pos.getX(a) + pos.getX(b) + pos.getX(c)) / 3; sum[1] += (pos.getY(a) + pos.getY(b) + pos.getY(c)) / 3;
        sum[2] += cz; sum[3]++;
      }
      subsets.forEach((subset, c) => {
        if (!subset.length) return;
        const geometry = new THREE.BufferGeometry();
        for (const [name, attribute] of Object.entries(source.attributes)) {
          if (name !== "battleOpacity") geometry.setAttribute(name, attribute);
        }
        geometry.setIndex(subset);
        geometry.setAttribute("battleOpacity", this.chunks[c].opacity);
        geometry.setAttribute("battleChar", this.chunks[c].char);
        const material = (mesh.material as THREE.Material).clone();
        instanceWreck(material);
        const instanced = new THREE.InstancedMesh(geometry, material, BATTLE_WRECK_INSTANCES);
        const first = this.chunks[c].meshes[0];
        if (first) instanced.instanceMatrix = first.instanceMatrix;
        else instanced.instanceMatrix.setUsage(THREE.DynamicDrawUsage);
        instanced.frustumCulled = false;
        instanced.count = 0;
        this.chunks[c].meshes.push(instanced);
        this.chunks[c].triangles += subset.length / 3;
        this.group.add(instanced);
      });
    }
    this.chunks.forEach((chunk, c) => {
      const sum = sums[c];
      if (sum[3]) chunk.pivot.set(sum[0] / sum[3], sum[1] / sum[3], sum[2] / sum[3]);
    });
  }

  /** Pose this hull type's sections; returns the ids actually drawn. */
  update(pieces: readonly BattleWreckPiece[], width: number, height: number, tilt = BATTLE_MODEL_TILT, depthSlope = 0): string[] {
    const shown: string[] = [];
    this.chunks.forEach((chunk, c) => {
      let count = 0;
      for (const piece of pieces) {
        if (piece.chunk !== c || count >= BATTLE_WRECK_INSTANCES || !chunk.meshes.length) continue;
        battleHullMatrix(piece as unknown as BattleModelShip, width, height, this.matrix, tilt, depthSlope);
        const [ox, oy, oz] = piece.offset, [pitch, yaw, roll] = piece.tumble, p = chunk.pivot;
        this.euler.set(pitch, yaw, roll);
        this.turn.makeRotationFromEuler(this.euler);
        this.local.makeTranslation(ox + p.x, oy + p.y, oz + p.z).multiply(this.turn);
        this.turn.makeTranslation(-p.x, -p.y, -p.z);
        this.local.multiply(this.turn);
        this.matrix.multiply(this.local);
        chunk.meshes[0].setMatrixAt(count, this.matrix);
        chunk.opacity.setX(count, piece.alpha);
        chunk.char.setX(count, piece.char);
        shown.push(piece.id);
        count++;
      }
      chunk.opacity.needsUpdate = chunk.char.needsUpdate = true;
      if (chunk.meshes.length) chunk.meshes[0].instanceMatrix.needsUpdate = true;
      for (const mesh of chunk.meshes) mesh.count = count;
    });
    this.group.visible = shown.length > 0;
    return shown;
  }

  dispose(): void {
    this.group.removeFromParent();
    for (const chunk of this.chunks) for (const mesh of chunk.meshes) {
      mesh.dispose(); mesh.geometry.dispose(); (mesh.material as THREE.Material).dispose();
    }
  }
}
