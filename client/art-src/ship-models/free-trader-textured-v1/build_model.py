"""Build the approved Free Trader with the shared fleet reconstruction pipeline.

No Blender and no new concept art. Native sprite pixels supply dorsal detail;
the five-view sheet supplies dedicated ventral/elevation/cap textures. Author
coordinates are plan X/Y plus height Z; the shared exporter uses Y up, -Z front.
"""
from pathlib import Path
import copy
import json
import math
import shutil
import sys

import numpy as np
from PIL import Image

ROOT = Path(__file__).resolve().parent
sys.path.insert(0, str(ROOT.parent / 'mining-fleet-v1'))
from reconstruction import Reconstruction

SPEC = dict(
    id='free-trader', name='Free Trader', center=[254, 436],
    pixelsPerUnit=[38, 38],
    reference='free-trader-reference-v1/free-trader-reference.png',
    sprite='free-trader-reference-v1/free-trader.png',
    triangleBudget=[6000, 8000],
    counts=dict(canopyPanels=8, dockingCollars=4, mainEngines=2, bridges=1, guns=0),
    regions=dict(top=[28, 55, 480, 816], bottom=[529, 50, 986, 816],
                 front=[1124, 63, 1619, 271], back=[1061, 337, 1682, 530],
                 side=[1015, 598, 1754, 799]),
    patches=dict(silver=[752, 1000, 1024, 1272], dark=[1040, 1000, 1312, 1272],
                 brass=[1328, 1000, 1600, 1272], enginewall=[1040, 1000, 1312, 1272],
                 glass=[198, 126, 300, 179], cyan=[1194, 453, 1205, 467],
                 purple=[1630, 1000, 1840, 1050], cream=[1630, 1070, 1840, 1120]),
    highResolutionTop=dict(sourceBounds=[277, 36, 977, 1207],
                           crop=[269, 28, 985, 1215], atlasOrigin=[0, 850]),
)


def extend_edges(rgb, known, iterations=32):
    """Native-pixel atlas padding only; no image enlargement or artistic edits."""
    tile = np.asarray(rgb, np.float32).copy()
    known = np.asarray(known, bool).copy()
    for _ in range(iterations):
        weighted = np.pad(tile * known[:, :, None], ((1, 1), (1, 1), (0, 0)))
        valid = np.pad(known.astype(np.float32), 1)
        total = np.zeros_like(tile)
        count = np.zeros_like(known, np.float32)
        h, w = known.shape
        for dy, dx in ((0, 1), (1, 0), (1, 2), (2, 1), (0, 0), (0, 2), (2, 0), (2, 2)):
            total += weighted[dy:dy+h, dx:dx+w]
            count += valid[dy:dy+h, dx:dx+w]
        fill = ~known & (count > 0)
        tile[fill] = total[fill] / count[fill, None]
        known |= fill
    tile[~known] = (56, 61, 68)
    return tile


class Trader(Reconstruction):
    def prepare_textures(self):
        folder = self.root / 'textures'
        folder.mkdir(exist_ok=True)
        art = self.root.parents[1]
        reference = art / self.spec['reference']
        sprite_path = art / self.spec['sprite']
        shutil.copy2(reference, folder / 'reference-sheet.png')
        shutil.copy2(sprite_path, self.root / 'concept.png')
        rgb = np.asarray(Image.open(reference).convert('RGB'), np.float32)
        sprite = np.asarray(Image.open(sprite_path).convert('RGBA'))
        atlas = np.full((2048, 2048, 3), (56, 61, 68), np.float32)
        for x0, y0, x1, y1 in self.spec['regions'].values():
            tile = rgb[y0:y1, x0:x1]
            atlas[y0:y1, x0:x1] = extend_edges(tile, tile.min(axis=2) < 237)
        hi = self.spec['highResolutionTop']
        x0, y0, x1, y1 = hi['crop']
        tile = sprite[y0:y1, x0:x1]
        ax, ay = hi['atlasOrigin']
        atlas[ay:ay+tile.shape[0], ax:ax+tile.shape[1]] = extend_edges(tile[:, :, :3], tile[:, :, 3] > 32)
        # Existing fleet material swatches cover hidden fittings at native size.
        # These are cropped (never enlarged) from the approved shared strip.
        strip = np.asarray(Image.open(ROOT.parent / 'mining-fleet-v1/material-strip.png').convert('RGB'))
        for key, sx in [('silver', 40), ('dark', 780), ('brass', 1510)]:
            tx, ty, ex, ey = self.spec['patches'][key]
            atlas[ty:ey, tx:ex] = strip[100:100+ey-ty, sx:sx+ex-tx]
        for key, crop in [('purple', (500, 555, 710, 605)), ('cream', (500, 490, 710, 540))]:
            tx, ty, ex, ey = self.spec['patches'][key]
            x0, y0, x1, y1 = crop
            atlas[ty:ey, tx:ex] = sprite[y0:y1, x0:x1, :3]
        r, g, b = atlas.transpose(2, 0, 1)
        cyan = np.clip((g-r-25)/100, 0, 1) * np.clip((b-r-30)/100, 0, 1)
        purple = (b > r*1.18) & (r > g*1.15) & (b > 85)
        cyan[purple] = 0  # Royal purple is paint, never emissive.
        brass = (r > b*1.3) & (r > 95) & (g > b*1.12)
        value = atlas.mean(axis=2)/255
        rough = np.where(value > .52, .56, .49)
        metal = np.where(value > .52, .28, .45)
        rough[brass], metal[brass] = .30, .72
        rough[purple], metal[purple] = .54, .08
        rough[cyan > .3], metal[cyan > .3] = .23, .08
        Image.fromarray(atlas.clip(0, 255).astype('uint8')).save(folder / self.textures[0])
        Image.fromarray((np.stack([np.ones_like(r), rough, metal], axis=2)*255).astype('uint8')).save(folder / self.textures[1])
        Image.fromarray((atlas*cyan[:, :, None]).clip(0, 255).astype('uint8')).save(folder / self.textures[2])
        self.materials[0]['name'] = 'Reference silver, brass and painted market canopy'

    def plan_uv(self, points, bottom=False):
        if bottom:
            return super().plan_uv(points, True)
        sx, sy = self.spec['pixelsPerUnit']
        cx, cy = self.spec['center']
        pixels = np.column_stack([cx+points[:, 0]*sx, cy-points[:, 1]*sy])
        top = np.array(self.spec['regions']['top']).reshape(2, 2)
        hi = self.spec['highResolutionTop']
        bounds = np.array(hi['sourceBounds']).reshape(2, 2)
        sprite = bounds[0]+(pixels-top[0])*(bounds[1]-bounds[0])/(top[1]-top[0])
        return (sprite-np.array(hi['crop'][:2])+np.array(hi['atlasOrigin']))/2048

    def mesh(self, name, vertices, faces, style='auto', mapping=None, smooth=False, assembly='hull'):
        if mapping is None and style == 'auto':
            def mapping(p, n, fi):
                if abs(n[2]) > .55:
                    return self.plan_uv(p, n[2] < 0)
                if name.startswith('bridge.glazing'):
                    return self.face_uv(p, n, 'glass')
                if name.startswith('canopy.panel.'):
                    panel = int(name.rsplit('.', 1)[1])
                    return self.face_uv(p, n, 'cream' if panel % 2 else 'purple')
                if name.startswith(('hull.', 'ventral.', 'market.')) and abs(n[0]) > .55:
                    plan_y = self.spec['center'][1]-p[:, 1]*38
                    # Register the two lateral collars to their actual stations,
                    # so their elevation pixels sit underneath the solid collars.
                    u = np.interp(plan_y, [55, 331, 489, 816], [1022, 1264, 1454, 1748])
                    v = np.clip(721-p[:, 2]*48, 606, 790)
                    if name == 'hull.streamlined-bow' or name.endswith('.bridge-cheek'):
                        # The elevated cabin owns all glazing: keep the bow's
                        # walls below the windows, without a second printed cabin.
                        v = np.clip(721-p[:, 2]*24, 687, 755)
                    return np.column_stack([u, v])/2048
                if name in ('hull.market-deck', 'ventral.cargo-keel', 'market.canopy-foundation'):
                    return self.face_uv(p, n, 'dark')
                return self.face_uv(p, n, 'silver')
        elif mapping is not None and name.startswith('engine.') and name.endswith('.housing'):
            original_mapping = mapping
            def mapping(p, n, fi):
                # A machinery-textured forward cap, distinct from the aft nozzle.
                if n[1] > .8:
                    return self.face_uv(p, n, 'dark')
                return original_mapping(p, n, fi)
        super().mesh(name, vertices, faces, style, mapping, smooth, assembly)

    def export(self):
        super().export()
        path = self.root / 'model-info.json'
        info = json.loads(path.read_text())
        info.update(textures='Native 1254px sprite dorsal crop and five-view ventral/elevation/cap pixels; native cropped existing fleet swatches on hidden fittings. No upscaling or newly generated art.',
                    sourceImages=[self.spec['sprite'], self.spec['reference']],
                    paintEmissive=False,
                    note='Concept reconstruction: top-view footprint is authoritative where generated elevations disagree. Unarmed static hull, six attachment sockets; gameplay integration is separate.')
        path.write_text(json.dumps(info, indent=2)+'\n')


def bow(m):
    stations = [(57, 16, -.10, .78), (89, 42, -.40, 1.16),
                (136, 61, -.65, 1.58), (198, 65, -.78, 1.37),
                (246, 48, -.85, 1.05)]
    vertices = []
    for py, width, lo, hi in stations:
        for x, z in [(-width*.75, hi), (width*.75, hi), (width, hi-.2),
                     (width, lo+.2), (width*.75, lo), (-width*.75, lo),
                     (-width, lo+.2), (-width, hi-.2)]:
            vertices.append(m.point(254+x, py, z))
    faces = [tuple(reversed(range(8))), tuple(range(32, 40))]
    faces += [(k*8+i, k*8+(i+1)%8, (k+1)*8+(i+1)%8, (k+1)*8+i)
              for k in range(4) for i in range(8)]
    m.mesh('hull.streamlined-bow', vertices, faces)
    m.slab('bridge.glazing', [(223, 114), (279, 114), (304, 139), (304, 166),
           (278, 184), (230, 184), (204, 166), (204, 139)], 1.25, 2.02, .22)
    # The glazing is a raised solid cabin with the native brass window mullions.
    for side, label in ((-1, 'port'), (1, 'starboard')):
        mirror = lambda x: 254+side*(x-254)
        m.slab(f'hull.{label}.bridge-cheek', [(mirror(x), y) for x, y in
               [(302, 175), (318, 173), (326, 218), (305, 246), (291, 224)]],
               .80, 1.53, .09)


def canopy_panel(m, index, y0, y1):
    # Eight separately closed, gently vaulted panels. The seams have real depth.
    left, right = 151, 358
    xs = np.linspace(left, right, 9)
    profile = [(x, 1.62+.42*(1-((x-254.5)/103.5)**2)) for x in xs]
    profile += [(right, 1.39), (left, 1.39)]
    n = len(profile)
    vertices = []
    for y, inset, drop in [(y0, 1.5, .045), (y0+1.5, 0, 0), (y1-1.5, 0, 0), (y1, 1.5, .045)]:
        for x, z in profile:
            x = 254.5+(x-254.5)*(1-inset/103.5)
            vertices.append(m.point(x, y, z-drop if z > 1.4 else z))
    faces = [tuple(reversed(range(n))), tuple(range(3*n, 4*n))]
    faces += [(k*n+i, k*n+(i+1)%n, (k+1)*n+(i+1)%n, (k+1)*n+i)
              for k in range(3) for i in range(n)]
    m.mesh(f'canopy.panel.{index+1}', vertices, faces)


def build(m):
    p = m.point
    bow(m)
    m.slab('hull.market-deck', [(128, 234), (380, 234), (412, 273), (412, 524),
           (380, 559), (324, 589), (184, 589), (128, 559), (96, 524), (96, 273)],
           -1.03, 1.19, .16)
    m.slab('ventral.cargo-keel', [(152, 262), (356, 262), (388, 296), (388, 518),
           (347, 561), (305, 578), (203, 578), (161, 561), (120, 518), (120, 296)],
           -1.24, -.89, .13)
    m.box('market.canopy-foundation', (144, 264, 365, 584), 1.05, 1.48, 'dark', .11, 12)
    for i, (a, b) in enumerate([(272, 307), (311, 347), (351, 386), (390, 427),
                               (431, 467), (471, 508), (512, 548), (552, 579)]):
        canopy_panel(m, i, a, b)
    for side, label in ((-1, 'port'), (1, 'starboard')):
        mirror = lambda x: 254+side*(x-254)
        m.slab(f'market.{label}.armored-margin', [(mirror(x), y) for x, y in
               [(363, 263), (383, 250), (410, 281), (410, 524), (384, 553),
                (369, 579), (356, 580), (366, 538)]], .83, 1.50, .09)
        m.beam(f'market.{label}.brass-canopy-rail', p(mirror(362), 282, 1.54),
               p(mirror(362), 559, 1.54), .07, .09, 'brass')
        for index, py in enumerate((277, 346, 422, 497, 572)):
            x = mirror(370)
            m.beam(f'market.{label}.rim-clamp.{index}', p(x, py-4, 1.54),
                   p(x, py+4, 1.54), 10/38, .18, 'brass')
        m.beam(f'market.{label}.ventral-rail', p(mirror(351), 294, -1.25),
               p(mirror(351), 541, -1.25), .07, .10, 'dark')
        # Four circular docking doors face sideways, with real annular collars.
        for j, py in enumerate((331, 489)):
            number = j+1 if side < 0 else j+3
            c = p(mirror(410), py, .48)
            axis = np.array([side, 0, 0])
            m.lathe(f'docking.{number}.housing', c, axis,
                    [(0, .68), (.15, .73), (.31, .60), (1.10, .60),
                     (1.23, .74), (1.48, .74), (1.65, .61), (1.84, .59)],
                    20, 'auto', 'brass', cap=(1264, 696, 25))
            m.lathe(f'docking.{number}.brass-rim', c, axis,
                    [(1.47, .76), (1.70, .76), (1.70, .57), (1.47, .57)],
                    20, 'brass', 'brass', ring=True)
            m.lathe(f'docking.{number}.recessed-door', c, axis,
                    [(1.72, .565), (1.75, .565)], 20, 'dark', 'dark', cap=(1264, 696, 24))
            m.socket(f'socket.docking.{number}', c+axis*1.86, axis, 'customer-shuttle-docking',
                     side=label, station='forward' if j == 0 else 'aft')
            for k in range(4):
                angle = math.tau*k/4+math.pi/4
                q = c+axis*1.70+np.array([0, math.cos(angle)*.67, math.sin(angle)*.67])
                m.cylinder(f'docking.{number}.rim-fastener.{k}', q, q+axis*.055, .047, 'dark', 6)
    # A thin aft waist leaves the pylon/engine gaps genuinely open.
    m.slab('hull.narrow-aft-waist', [(218, 574), (290, 574), (295, 701),
           (277, 755), (231, 755), (213, 701)], -.71, .92, .11)
    m.box('hull.aft-dorsal-spine', (235, 588, 273, 733), .84, 1.08, bevel=.06, corner=8)
    for side, label in ((-1, 'port'), (1, 'starboard')):
        mirror = lambda x: 254+side*(x-254)
        m.slab(f'hull.{label}.swept-pylon', [(mirror(x), y) for x, y in
               [(295, 591), (325, 587), (398, 650), (388, 674), (351, 657), (295, 626)]],
               -.47, .54, .09)
        m.beam(f'hull.{label}.pylon-brass-brace', p(mirror(306), 602, .60),
               p(mirror(378), 650, .60), .20, .13, 'brass')
        m.cylinder(f'machinery.{label}.coolant-reservoir', p(mirror(303), 585, .49),
                   p(mirror(303), 651, .49), .24, 'silver', 16)
        m.cylinder(f'machinery.{label}.pylon-conduit', p(mirror(329), 565, .25),
                   p(mirror(365), 641, .10), .12, 'dark', 12)
        center = p(mirror(395), 645, .26)
        axis = np.array([0, -1, 0])
        m.lathe(f'engine.{label}.housing', center, axis,
                [(0, .72), (.18, 1.0), (.48, 1.20), (.82, 1.32),
                 (2.20, 1.32), (2.47, 1.22), (2.77, 1.20),
                 (3.24, 1.10), (3.70, .99), (4.25, .87)],
                32, 'auto', 'enginewall', cap=(1198, 460, 47))
        for j, height in enumerate((.60, 2.27)):
            radius = 1.31 if j == 0 else 1.35
            m.lathe(f'engine.{label}.brass-band.{j}', center, axis,
                    [(height, radius), (height+.13, radius),
                     (height+.13, radius-.08), (height, radius-.08)],
                    24, 'brass', 'brass', ring=True)
        m.lathe(f'engine.{label}.nozzle-rim', center, axis,
                [(4.05, .94), (4.39, .89), (4.39, .70), (4.05, .73)],
                32, 'dark', 'enginewall', ring=True)
        m.lathe(f'engine.{label}.aperture', center, axis,
                [(4.26, .69), (4.29, .69)], 32, 'dark', 'dark', cap=(1198, 460, 30))
        m.socket(f'socket.engine.{label}', center+axis*4.40, axis, 'engine-exhaust')
        ex = mirror(395)
        m.box(f'engine.{label}.dorsal-service-plate', (ex-12, 663, ex+12, 721),
              1.51, 1.68, bevel=.045, corner=3)
        # Subtle cylinder-side protection bars add form without covering the paint.
        for k, sign in enumerate((-1, 1)):
            m.beam(f'engine.{label}.protection-bar.{k}', center+[sign*1.19, -.80, .50],
                   center+[sign*1.19, -2.15, .50], .075, .10, 'brass')
    for j, py in enumerate((316, 402, 488)):
        m.box(f'ventral.cargo-hatch.{j}', (172, py, 336, py+62), -1.39, -1.18, bevel=.055, corner=7)
    m.box('ventral.bow-service-hatch', (228, 184, 281, 238), -.90, -.74, bevel=.025, corner=6)
    m.box('ventral.aft-service-hatch', (232, 636, 276, 700), -.86, -.67, bevel=.03, corner=4)
    # Four market-frame corners have clearly machined, closed bolt caps.
    for j, (x, y) in enumerate(((133, 283), (375, 283), (133, 522), (375, 522))):
        m.lathe(f'market.frame-fastener.{j}', p(x, y, 1.50), [0, 0, 1],
                [(0, .11), (.055, .11), (.07, .08)], 12, 'dark', 'dark')


if __name__ == '__main__':
    model = Trader(ROOT, copy.deepcopy(SPEC))
    model.prepare_textures()
    build(model)
    model.export()
