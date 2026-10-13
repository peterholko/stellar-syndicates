"""Build an attack-disabled, repairable Nexus variant without Blender."""
from pathlib import Path
import math
import shutil
import numpy as np
from PIL import Image
import build_model as b

ROOT = Path(__file__).resolve().parent
OUTPUT = ROOT/'disabled'


def prepare_materials():
    folder = OUTPUT/'textures'
    folder.mkdir(parents=True, exist_ok=True)
    generated = {b.TEXTURES[i] for i in (3, 5, 7)}
    for name in b.TEXTURES:
        if name not in generated:
            shutil.copy2(ROOT/'textures'/name, folder/name)
        else:
            assert (folder/name).exists(), f'Missing generated damage texture: {name}'
    # PBR data maps derived from the generated albedos. Darkened fractures are
    # rougher and less metallic than clean surviving armor and brass.
    for albedo_index, mask_index, metal in [(3, 4, .32), (5, 6, .78), (7, 8, .35)]:
        pixels = np.asarray(Image.open(folder/b.TEXTURES[albedo_index]).convert('RGB'), float)/255
        light = pixels@np.array([.2126, .7152, .0722])
        soot = np.clip((.48-light)*2, 0, 1)
        packed = np.ones_like(pixels)
        packed[:, :, 1] = .58+.30*soot
        packed[:, :, 2] = metal*(1-.65*soot)
        Image.fromarray((packed*255).astype('uint8')).save(folder/b.TEXTURES[mask_index])
    for i in (2, 9):
        Image.new('RGB', (1, 1), 'black').save(folder/b.TEXTURES[i])
    for material in b.materials:
        material.pop('emissiveTexture', None)
        material['emissiveFactor'] = [0, 0, 0]
    for i in (b.ATLAS, b.BELLY):
        b.materials[i]['pbrMetallicRoughness']['baseColorFactor'] = [.56, .53, .50, 1]
    b.materials[b.IVORY]['name'] = 'Impact-scorched ivory armor'
    b.materials[b.IVORY]['pbrMetallicRoughness']['baseColorFactor'] = [.88, .86, .83, 1]
    b.materials[b.COLLAR]['name'] = 'Scorched brass containment collars'
    b.materials[b.COLLAR]['pbrMetallicRoughness']['baseColorFactor'] = [.70, .65, .58, 1]
    b.materials[b.POWER]['name'] = 'Fractured reactor cells / unpowered'
    b.materials[b.POWER]['pbrMetallicRoughness']['baseColorFactor'] = [.60, .63, .65, 1]
    b.materials[b.BRASS]['pbrMetallicRoughness']['baseColorFactor'] = [.70, .65, .58, 1]
    b.materials[b.AMBER]['name'] = 'Emergency amber markers / reserve power'
    b.materials[b.AMBER]['pbrMetallicRoughness']['baseColorFactor'] = [.19, .070, .009, 1]
    b.materials[b.AMBER]['emissiveFactor'] = [.70, .21, .028]


def fractured_shell(side):
    angle = math.pi/2
    axis, tangent = b.polar(1, angle, 0), b.polar(1, angle+math.pi/2, 0)
    # An extruded, jagged fracture profile produces closed geometry rather
    # than hiding the upper tower with a plane or leaving uncapped faces.
    profile = [(5, .44), (5.1, 1.05), (5.72, 1.40), (6.75, 2.10),
               (7.02, 2.48), (7.25, 2.38), (7.38, 2.68), (7.66, 2.46),
               (7.96, 2.59), (8.21, 2.31), (8.65, 2.47),
               (8.77, 2.10), (9.12, 1.40), (9.55, 1.05), (9.87, .44)]
    vertices = [axis*r+tangent*side*t+[0, 0, z] for t in (.36, .50) for r, z in profile]
    n = len(profile)
    faces = [tuple(reversed(range(n))), tuple(range(n, n*2))]
    faces.extend((i, (i+1) % n, (i+1) % n+n, i+n) for i in range(n))

    def uv(p, normal):
        r, t, z = float(p@axis), float(p@tangent)-side*.43, p[2]
        dominant = np.argmax([abs(normal@axis), abs(normal@tangent), abs(normal[2])])
        if dominant == 1:
            return (.02+.96*(r-5)/4.87, .98-.96*(z-.44)/4.21)
        if dominant == 2:
            return (.02+.96*(r-5)/4.87, .5-.96*t/4.21)
        return (.5+.96*t/4.87, .98-.96*(z-.44)/4.21)

    label = 'left' if side < 0 else 'right'
    b.mesh(f'stabilizer-2.fractured-{label}-shell', vertices, faces, b.IVORY, 'stabilizer-2', uv)


def impact_crater(index, angle, width, depth, burn, steel):
    axis, tangent = b.polar(1, angle, 0), b.polar(1, angle+math.pi/2, 0)
    center = b.polar(8.25, angle, .66)
    irregular = [1, .86, 1.06, .91, 1.02, .89, 1.08, .92]
    profile = [(1, -.015), (1, .065), (.68, .14), (.52, .018)]
    vertices = [center+axis*math.cos(i*b.TAU/8)*width*r*irregular[i]+
                tangent*math.sin(i*b.TAU/8)*depth*r*irregular[i]+[0, 0, z]
                for r, z in profile for i in range(8)]
    faces = [tuple(reversed(range(8))), tuple(range(24, 32))]
    for row in range(3):
        faces.extend((row*8+i, row*8+(i+1) % 8, (row+1)*8+(i+1) % 8, (row+1)*8+i) for i in range(8))
    def impact_uv(p, n):
        q = p-center
        if abs(n[2]) > .4:
            return (.5+q@axis/(width*2.2), .5+q@tangent/(depth*2.2))
        u = q@tangent/(depth*2.2) if abs(n@axis) > abs(n@tangent) else q@axis/(width*2.2)
        return (.5+u, .15+q[2]*3)
    b.mesh(f'attack.impact-{index}.scorched-crater', vertices, faces, burn, mapping=impact_uv)
    for j in range(2):
        t = (j-.5)*.29
        start = center+tangent*t-axis*.36+[0, 0, .034]
        middle = center+tangent*(t+.10)+axis*.025+[0, 0, .09]
        end = center+tangent*(t-.035)+axis*.34+[0, 0, .038]
        b.beam(f'attack.impact-{index}.exposed-rib-{j}-a', start, middle, .035, .028, steel)
        b.beam(f'attack.impact-{index}.exposed-rib-{j}-b', middle, end, .035, .028, steel)
    for j in (0, 2, 5):
        a = j*b.TAU/8
        p = center+axis*math.cos(a)*width*.81+tangent*math.sin(a)*depth*.81
        b.beam(f'attack.impact-{index}.torn-plate-{j}', p+[0, 0, .04],
               p+tangent*.22+axis*.18+[0, 0, .21], .14, .038, b.IVORY)
        g = b.parts[-1]['groups'][b.IVORY]
        for start in range(0, len(g['position']), 4):
            points = np.asarray(g['position'][start:start+4])
            axes = [k for k in range(3) if k != np.argmax(np.abs(g['normal'][start]))]
            local = points[:, axes]
            g['uv'][start:start+4] = (.04+.92*(local-local.min(axis=0))/np.maximum(np.ptp(local, axis=0), 1e-8)).tolist()


def main():
    prepare_materials()
    b.parts.clear()
    b.build_geometry()
    def retain(part):
        name = part['name']
        if part['assembly'] in ('wormhole', 'containment-field'):
            return False
        if name in ('cargo.container.3', 'cargo.container.14'):
            return False
        if name.startswith('stabilizer-2.'):
            suffix = name.split('.', 1)[1]
            if suffix.startswith(('left-shell', 'right-shell')) or suffix in ('powered-core', 'crown', 'beacon'):
                return False
            if suffix.startswith('coil.') and int(suffix.rsplit('.', 1)[1]) >= 3:
                return False
        return True
    b.parts[:] = [part for part in b.parts if retain(part)]
    for part in b.parts:
        if part['name'].endswith('.beacon'):
            part['groups'][b.AMBER] = part['groups'].pop(b.POWER)
    fractured_shell(-1)
    fractured_shell(1)
    angle = math.pi/2
    b.cylinder('stabilizer-2.broken-powered-core', b.polar(8.03, angle, 1.09),
               b.polar(8.03, angle, 2.52), .205, b.POWER, 'stabilizer-2', 12, uv_style='power')
    b.TEXTURES.append('authority-nexus-impact.png')
    burn = b.material('Scorched impact recesses / exposed machinery', (.64, .60, .55), .30, .89)
    b.materials[burn]['pbrMetallicRoughness']['baseColorTexture'] = {'index': len(b.TEXTURES)-1}
    steel = b.material('Exposed torn titanium', (.38, .43, .48), .70, .67)
    b.materials[steel]['pbrMetallicRoughness']['baseColorTexture'] = {'index': 3}
    b.cylinder('stabilizer-2.exposed-core-cap', b.polar(8.03, angle, 2.515),
               b.polar(8.03, angle, 2.55), .212, steel, 'stabilizer-2', 12)
    impact_crater(1, math.pi/4+.06, 1.0, .80, burn, steel)
    impact_crater(2, math.pi*1.20, .83, .64, burn, steel)
    for index, a in enumerate([.32, 2.83, 3.46, 5.97]):
        b.cylinder(f'emergency.inner-marker-{index}', b.polar(4.84, a, 1.01),
                   b.polar(4.84, a, 1.08), .065, b.AMBER, segments=6)
    b.ROOT = OUTPUT
    b.export('authority-nexus-disabled', {
        'name': 'Authority Nexus — Attack disabled', 'state': 'attack-disabled',
        'texturing': 'Impact-scorched ivory and brass; textured graphite supports and exposed titanium; repaired local UVs on all physical faces; fractured unpowered reactor cells; seven amber reserve-power markers',
        'damage': ['Stabilizer 2 fractured and capped', 'Two scarred impact recesses with exposed ribs', 'Portal and containment field absent'],
        'note': 'Repairable disabled-state asset. No active portal or reopening animation. Not game-integrated.',
    })


if __name__ == '__main__':
    main()
