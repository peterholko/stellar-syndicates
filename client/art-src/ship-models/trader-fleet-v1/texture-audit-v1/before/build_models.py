"""Reconstruct the three approved trader concepts with the existing fleet pipeline.

Author coordinates follow the TOP drawing: sheet X/Y plus model-unit height.
Exported coordinates are Y up, -Z forward. Native reference pixels are packed
without enlargement; real closed geometry defines every silhouette and opening.
"""
from pathlib import Path
import argparse
import copy
import hashlib
import json
import math
import shutil
import sys

import numpy as np
from PIL import Image

ROOT = Path(__file__).resolve().parent
sys.path.insert(0, str(ROOT.parent / 'mining-fleet-v1'))
from reconstruction import Reconstruction


SPECS = {
    'clean-slate': dict(name='Clean Slate', owner='Ines Varga', center=[246, 436],
        regions=dict(top=[22, 58, 470, 815], bottom=[513, 58, 953, 815],
                     front=[1057, 71, 1662, 277], back=[1052, 352, 1670, 545],
                     side=[958, 618, 1754, 815]),
        sideBaseline=755, sideHeightScale=44, sideRegistration=[[58, 435, 815], [965, 1385, 1750]],
        frontBaseline=228, backBaseline=490, elevationScale=46,
        paintCrop=[140, 275, 204, 363], accentCrop=[403, 293, 412, 345],
        glassCrop=[179, 145, 191, 155], cyanCrop=[1222, 476, 1241, 481],
        engineWallCrop=[1551, 710, 1700, 755],
        counts=dict(cargoBays=6, dockingCollars=2, mainEngines=2, bridges=1, cranes=0, guns=0)),
    'second-wind': dict(name='Second Wind', owner='Mags Halloran', center=[278, 437],
        regions=dict(top=[30, 56, 533, 819], bottom=[550, 55, 979, 820],
                     front=[1105, 24, 1715, 271], back=[1124, 290, 1712, 543],
                     side=[1021, 571, 1750, 815]),
        sideBaseline=748, sideHeightScale=42, sideRegistration=[[56, 275, 437, 819], [1030, 1147, 1226, 1745]],
        frontBaseline=219, backBaseline=475, elevationScale=42,
        starboardSide=[1284, 680, 1592, 793], engineSide=[1607, 666, 1748, 802],
        paintCrop=[236, 273, 295, 325], accentCrop=[232, 373, 240, 425],
        glassCrop=[125, 297, 137, 307], cyanCrop=[1224, 467, 1241, 481],
        engineWallCrop=[1449, 703, 1585, 756],
        counts=dict(cargoBays=1, dockingCollars=1, mainEngines=2, bridges=1, cranes=1, guns=0)),
    'small-fortune': dict(name='Small Fortune', owner='Rafi Mensah', center=[262, 429],
        regions=dict(top=[36, 47, 490, 810], bottom=[533, 48, 980, 811],
                     front=[1067, 49, 1681, 280], back=[1062, 327, 1682, 540],
                     side=[1019, 608, 1756, 800]),
        sideBaseline=753, sideHeightScale=42, sideRegistration=[[47, 321, 810], [1025, 1198, 1750]],
        frontBaseline=229, backBaseline=478, elevationScale=42,
        starboardSide=[1234, 630, 1420, 779], engineSide=[1486, 638, 1752, 783],
        paintCrop=[164, 268, 223, 326], accentCrop=[251, 273, 272, 330],
        glassCrop=[269, 162, 281, 172], cyanCrop=[1155, 473, 1170, 484],
        engineWallCrop=[1556, 714, 1680, 754],
        counts=dict(cargoBays=4, dockingCollars=1, mainEngines=2, bridges=1, cranes=0, guns=0)),
}
for slug, spec in SPECS.items():
    spec.update(id=slug, pixelsPerUnit=[38, 38],
        reference=f'{slug}-reference-v1/{slug}-reference.png',
        triangleBudget=[3500, 11000], patches={})


def extend_edges(rgb, iterations=28):
    """Pad atlas margins for texture filtering; retain native source texels."""
    tile = np.asarray(rgb, np.float32).copy()
    known = tile.min(axis=2) < 237
    for _ in range(iterations):
        weighted = np.pad(tile * known[:, :, None], ((1, 1), (1, 1), (0, 0)))
        valid = np.pad(known.astype(np.float32), 1)
        total, count = np.zeros_like(tile), np.zeros_like(known, np.float32)
        h, w = known.shape
        for dy, dx in ((0, 1), (1, 0), (1, 2), (2, 1), (0, 0), (0, 2), (2, 0), (2, 2)):
            total += weighted[dy:dy+h, dx:dx+w]
            count += valid[dy:dy+h, dx:dx+w]
        fill = ~known & (count > 0)
        tile[fill] = total[fill] / count[fill, None]
        known |= fill
    tile[~known] = (52, 58, 65)
    return tile


class Trader(Reconstruction):
    def prepare_textures(self):
        folder = self.root / 'textures'
        folder.mkdir(parents=True, exist_ok=True)
        reference = self.root.parents[1] / self.spec['reference']
        shutil.copy2(reference, folder / 'reference-sheet.png')
        rgb = np.asarray(Image.open(reference).convert('RGB'), np.float32)
        atlas = np.full((2048, 2048, 3), (52, 58, 65), np.float32)
        for x0, y0, x1, y1 in self.spec['regions'].values():
            atlas[y0:y1, x0:x1] = extend_edges(rgb[y0:y1, x0:x1])
        strip_path = ROOT.parent / 'mining-fleet-v1/material-strip.png'
        strip = np.asarray(Image.open(strip_path).convert('RGB'), np.float32)
        patches = self.spec['patches']
        for i, (key, sx) in enumerate([('silver', 40), ('dark', 780), ('brass', 1510)]):
            x, y = i*272, 960
            atlas[y:y+256, x:x+256] = strip[100:356, sx:sx+256]
            patches[key] = [x+2, y+2, x+254, y+254]
        for i, (key, crop) in enumerate([
            ('paint', self.spec['paintCrop']), ('accent', self.spec['accentCrop']),
            ('glass', self.spec['glassCrop']), ('cyan', self.spec['cyanCrop']),
            ('enginewall', self.spec['engineWallCrop'])]):
            x0, y0, x1, y1 = crop
            x, y = 816+(i % 4)*272, 960+(i//4)*272
            h, w = y1-y0, x1-x0
            atlas[y:y+h, x:x+w] = rgb[y0:y1, x0:x1]
            patches[key] = [x+.5, y+.5, x+w-.5, y+h-.5]
        r, g, b = atlas.transpose(2, 0, 1)
        # Colored paint never drives emission. Only small bright cyan lights do.
        cyan = np.clip((g-r-35)/95, 0, 1)*np.clip((b-r-45)/95, 0, 1)*np.clip((b-135)/100, 0, 1)
        value = atlas.mean(axis=2)/255
        rough, metal = np.where(value > .52, .59, .48), np.where(value > .52, .22, .43)
        warm = (r > b*1.3) & (r > 110) & (g > b*1.15)
        rough[warm], metal[warm] = .43, .44
        rough[cyan > .3], metal[cyan > .3] = .23, .09
        for data, name in [
            (atlas, self.textures[0]),
            (np.stack([np.ones_like(r), rough, metal], axis=2)*255, self.textures[1]),
            (atlas*cyan[:, :, None], self.textures[2])]:
            Image.fromarray(data.clip(0, 255).astype('uint8')).save(folder / name)
        self.materials[0]['name'] = self.spec['name'] + ' / painted hull and fittings'
        self.materials[1]['name'] = 'Ventral / ' + self.spec['name']
        self.sources = [reference, strip_path]

    def elevation_uv(self,p,front):
        x0,y0,x1,y1=self.spec['regions']['front' if front else 'back']
        top=self.spec['regions']['top']
        px=self.spec['center'][0]+p[:,0]*38
        fraction=(px-top[0])/(top[2]-top[0])
        u=x1-fraction*(x1-x0) if front else x0+fraction*(x1-x0)
        v=np.clip(self.spec['frontBaseline' if front else 'backBaseline']-p[:,2]*self.spec['elevationScale'],y0+2,y1-2)
        if front and self.spec['id']=='small-fortune':
            # Its concept FRONT compresses the cabin relative to the engine
            # spacing. Keep bow texels inside the cabin, clear of white gaps.
            u=1540-fraction*332
            v=np.clip(218-p[:,2]*32,186,248)
        return np.column_stack([u,v])/2048

    def mesh(self, name, vertices, faces, style='auto', mapping=None, smooth=False, assembly='hull'):
        if mapping is None and style == 'auto':
            def mapping(p, n, fi):
                if abs(n[2]) > .55:
                    return self.plan_uv(p, n[2] < 0)
                if name == 'bridge.glazing':
                    return self.face_uv(p, n, 'glass')
                if name.startswith('bridge.'):
                    return self.face_uv(p, n, 'paint')
                if '.pylon.' in name or name.endswith(('engine-mount','loading-lock','loading-hatch')):
                    return self.face_uv(p,n,'paint' if 'loading' in name else 'dark')
                plan_y=self.spec['center'][1]-p[:,1]*38
                if name in ('hull.chisel-bow','hull.spade-body') and plan_y.mean()<240 and n[1]>-.05:
                    return self.elevation_uv(p,True)
                if name.startswith(('hull.', 'ventral.')) and abs(n[0]) > .55:
                    plan_y = self.spec['center'][1]-p[:, 1]*38
                    stations, pixels = self.spec['sideRegistration']
                    u = np.interp(plan_y, stations, pixels)
                    bounds = self.spec['regions']['side']
                    v = np.clip(self.spec['sideBaseline']-p[:, 2]*self.spec['sideHeightScale'], bounds[1]+3, bounds[3]-3)
                    # The raised bridge owns the windows, not the lower bow.
                    if 'bow' in name:
                        v = np.maximum(v, self.spec['sideBaseline']-8)
                    if n[0] > 0 and 'starboardSide' in self.spec:
                        # The only SIDE drawing shows port. Its round docking
                        # door must not be printed onto the starboard hull.
                        x0,y0,x1,y1=self.spec['starboardSide']
                        top=self.spec['regions']['top']
                        u=x0+np.clip((plan_y-top[1])/(top[3]-top[1]),0,1)*(x1-x0)
                        v=np.clip((y0+y1)/2-p[:,2]*self.spec['sideHeightScale'],y0+2,y1-2)
                    return np.column_stack([u, v])/2048
                if name.startswith(('hull.', 'ventral.')) and abs(n[1])>.55:
                    return self.elevation_uv(p,n[1]>0)
                return self.face_uv(p, n, 'paint' if name.startswith(('cargo.', 'armor.', 'cradle.')) else 'silver')
        elif mapping is not None and name.startswith('engine.') and name.endswith(('.housing','.inner-core')):
            original=mapping
            label=name.split('.')[1]
            settings=getattr(self,'engine_maps',{}).get(label)
            if settings:
                def mapping(p,n,fi):
                    if abs(n[2])>.65 and style=='auto':
                        return self.plan_uv(p,n[2]<0)
                    if n[1]>.8:
                        if self.spec['id']=='small-fortune':
                            cx,cy,radius=1137,205,42
                            return np.column_stack([cx+(p[:,0]-settings['center'][0])/settings['radius']*radius,
                                cy-(p[:,2]-settings['center'][2])/settings['radius']*radius])/2048
                        return self.face_uv(p,n,'dark')
                    if abs(n[1])<.8:
                        x0,y0,x1,y1=self.spec['engineSide']
                        py=self.spec['center'][1]-p[:,1]*38
                        u=x0+(py-settings['y0'])/(settings['y1']-settings['y0'])*(x1-x0)
                        v=np.clip((y0+y1)/2-(p[:,2]-settings['center'][2])*42,y0+2,y1-2)
                        return np.column_stack([u,v])/2048
                    return original(p,n,fi)
        super().mesh(name, vertices, faces, style, mapping, smooth, assembly)

    def export(self):
        super().export()
        path = self.root / 'model-info.json'
        info = json.loads(path.read_text())
        info.update(owner=self.spec['owner'],
            textures='Native approved five-view pixels, separate ventral projection, native palette crops and existing fleet metal swatches. Three embedded 2048px PBR maps; no upscaling.',
            note='Static concept reconstruction. TOP defines footprint and part stations; elevations guide height. Major reference features are geometry. Exact gameplay scale, LODs and game integration are separate.',
            sourceImages=[self.spec['reference']], paintEmissive=False)
        path.write_text(json.dumps(info, indent=2)+'\n')
        (self.root / 'source-integrity.json').write_text(json.dumps({
            'sources': [{'path': str(p), 'sha256': hashlib.sha256(p.read_bytes()).hexdigest()} for p in self.sources],
            'handling': 'Read-only references; unchanged copy in textures/reference-sheet.png. Native pixels packed into atlases.'}, indent=2)+'\n')


def loft(m, name, stations, style='auto'):
    """Eight-sided tapered hull sections with actual lower and upper bevels."""
    vertices = []
    for y, cx, width, low, high in stations:
        bevel = min(.28, (high-low)*.22)
        for dx, z in [(-width*.78, high), (width*.78, high), (width, high-bevel),
                       (width, low+bevel), (width*.78, low), (-width*.78, low),
                       (-width, low+bevel), (-width, high-bevel)]:
            vertices.append(m.point(cx+dx, y, z))
    n = len(stations)
    faces = [tuple(reversed(range(8))), tuple(range((n-1)*8, n*8))]
    faces += [(k*8+i, k*8+(i+1)%8, (k+1)*8+(i+1)%8, (k+1)*8+i)
              for k in range(n-1) for i in range(8)]
    m.mesh(name, vertices, faces, style)


def bolts(m, name, points, z, radius=.055, style='dark'):
    for i, (x, y) in enumerate(points):
        m.lathe(f'{name}.{i}', m.point(x, y, z), [0, 0, 1],
                [(0, radius), (.055, radius), (.065, radius*.78)], 8, style, style)


def docking(m, label, x, y, side, radius=.78, length=1.14, cap=None):
    c, axis = m.point(x, y, .05), np.array([side, 0, 0])
    m.lathe(f'docking.{label}.housing', c, axis,
        [(0, radius*.85), (.16, radius), (.31, radius*.86),
         (length-.28, radius*.86), (length-.19, radius), (length, radius)],
        24, 'auto', 'silver', cap=cap)
    m.lathe(f'docking.{label}.rim', c, axis,
        [(length-.14, radius*1.02), (length+.05, radius*1.02),
         (length+.05, radius*.77), (length-.14, radius*.77)],
        24, 'brass', 'brass', ring=True)
    m.lathe(f'docking.{label}.door', c, axis,
        [(length-.02, radius*.76), (length+.01, radius*.76)],
        24, 'dark', 'dark', cap=cap)
    m.socket(f'socket.docking.{label}', c+axis*(length+.07), axis, 'customer-shuttle-docking')
    for i in range(4):
        a = math.tau*i/4+math.pi/4
        q = c+axis*(length+.05)+np.array([0, math.cos(a)*radius*.88, math.sin(a)*radius*.88])
        m.cylinder(f'docking.{label}.latch.{i}', q, q+axis*.07, .07, 'dark', 8)


def runner_engine(m, label, cx, y0, y1, radius, height, cap, square=False, bands=True):
    c, axis = m.point(cx, y0, height), np.array([0, -1, 0])
    if not hasattr(m,'engine_maps'):m.engine_maps={}
    m.engine_maps[label]=dict(center=c,radius=radius,y0=y0,y1=y1)
    length = (y1-y0)/38
    rows = [(0, radius*.65), (.18, radius*.83), (.48, radius),
            (length*.23, radius), (length*.65, radius),
            (length*.72, radius*.94), (length*.79, radius*.83),
            (length*.88, radius*.74), (length*.97, radius*.65)]
    if square:
        m.box(f'engine.{label}.housing', (cx-radius*38, y0+5, cx+radius*38, y0+length*.71*38),
              height-radius*.85, height+radius*.94, bevel=.20, corner=12)
        m.lathe(f'engine.{label}.inner-core', c, axis, rows, 32, 'dark', 'dark', cap=cap)
    else:
        m.lathe(f'engine.{label}.housing', c, axis, rows, 32, 'auto', 'enginewall', cap=cap)
    if bands:
        for i, distance in enumerate((length*.18, length*.65)):
            m.lathe(f'engine.{label}.coupling-band.{i}', c, axis,
                [(distance, radius*1.025), (distance+.15, radius*1.025),
                 (distance+.15, radius*.96), (distance, radius*.96)],
                24, 'brass', 'brass', ring=True)
    for i, (distance, r) in enumerate([(length*.79, radius*.85), (length*.88, radius*.77)]):
        m.lathe(f'engine.{label}.nozzle-fin.{i}', c, axis,
            [(distance, r), (distance+.08, r), (distance+.08, r*.92), (distance, r*.92)],
            32, 'dark', 'dark', ring=True)
    m.lathe(f'engine.{label}.nozzle-rim', c, axis,
        [(length*.92, radius*.70), (length, radius*.65),
         (length, radius*.49), (length*.92, radius*.50)],
        32, 'dark', 'dark', ring=True)
    m.lathe(f'engine.{label}.aperture', c, axis,
        [(length*.96, radius*.485), (length*.97, radius*.485)],
        32, 'dark', 'dark', cap=(cap[0], cap[1], cap[2]*.74))
    m.socket(f'socket.engine.{label}', c+axis*(length+.015), axis, 'engine-exhaust')


def build_clean_slate(m):
    p = m.point
    loft(m, 'hull.chisel-bow', [(60, 246, 33, -.20, .64), (94, 246, 92, -.55, .90),
        (137, 246, 160, -.74, 1.05), (199, 246, 168, -.80, 1.0), (252, 246, 163, -.85, .90)])
    m.slab('hull.enclosed-freight-body', [(110,205),(383,205),(426,277),(426,608),
        (396,683),(375,760),(122,760),(96,680),(62,608),(62,277)], -.92, 1.04, .18)
    m.slab('hull.integrated-stern', [(106,645),(388,645),(427,706),(424,773),
        (400,786),(94,786),(67,773),(68,704)], -.81, .85, .20)
    m.box('hull.stern-center-post',(224,761,270,807),-.77,.82,bevel=.08,corner=4)
    m.slab('bridge.glazing', [(165,119),(325,119),(352,134),(346,167),
        (317,181),(174,181),(137,164),(137,136)], 1.0, 1.68, .16)
    m.box('bridge.roof', (168,97,324,125), .90, 1.10, bevel=.06, corner=8)
    for i, (x, y) in enumerate([(x,y) for x in (115,270) for y in (250,386,520)]):
        m.box(f'cargo.bay.{i+1}.coaming', (x-5,y-5,x+115,y+122), 1.0, 1.22, 'dark', .05, 8)
        m.box(f'cargo.bay.{i+1}.lid', (x,y,x+110,y+115), 1.18, 1.40, bevel=.06, corner=9)
        for j, hy in enumerate((y+17,y+91)):
            m.box(f'cargo.bay.{i+1}.hinge.{j}', (x-3,hy,x+3,hy+12), 1.33, 1.45, 'brass', .02, 1)
        bolts(m, f'cargo.bay.{i+1}.fastener', [(x+8,y+8),(x+102,y+8),(x+8,y+107),(x+102,y+107)], 1.41)
    m.box('machinery.central-spine', (224,192,266,697), .92, 1.20, bevel=.06, corner=9)
    for side, label in ((-1,'port'),(1,'starboard')):
        sx = lambda x: 246+side*(x-246)
        m.slab(f'armor.{label}.shoulder', [(sx(x),y) for x,y in
            [(390,238),(413,260),(426,289),(426,608),(400,663),(382,638)]], .86, 1.23, .06)
        docking(m,label,sx(426),435,side,.86,1.10,(1387,727,31))
        for i,y in enumerate((302,486,605)):
            m.box(f'armor.{label}.edge-clamp.{i}', (min(sx(417),sx(430)),y,max(sx(417),sx(430)),y+22),
                  .10,.63,'brass',.04,3)
        ex = sx(337)
        # Closed rectangular engine body; the rear outlet is a real recessed frame.
        m.box(f'engine.{label}.housing', (ex-53,700,ex+53,798), -.64,.79,bevel=.12,corner=10)
        x0,x1 = ex-48,ex+48
        for key,bounds,lo,hi in [
            ('top',(x0,790,x1,810),.42,.69), ('bottom',(x0,790,x1,810),-.66,-.39),
            ('left',(x0,790,x0+10,810),-.39,.42), ('right',(x1-10,790,x1,810),-.39,.42)]:
            m.box(f'engine.{label}.frame.{key}',bounds,lo,hi,'dark',.025,2)
        c = p(ex,803,.015)
        verts = [c+np.array([dx,dy,dz]) for dy in (-.02,.02)
                 for dx,dz in ((-.95,-.28),(.95,-.28),(.95,.28),(-.95,.28))]
        def outlet_uv(points, normal, fi, center=c):
            if normal[1]<-.8:
                u=1233+(points[:,0]-center[0])*45
                v=478-(points[:,2]-center[2])*36
                return np.column_stack([u,v])/2048
            return m.face_uv(points,normal,'dark')
        m.mesh(f'engine.{label}.aperture',verts,[(0,3,2,1),(4,5,6,7),(0,1,5,4),(1,2,6,5),(2,3,7,6),(3,0,4,7)],'dark',mapping=outlet_uv)
        m.socket(f'socket.engine.{label}',p(ex,812,.015),[0,-1,0],'engine-exhaust')
        m.box(f'engine.{label}.service-cover',(ex-25,718,ex+24,764),.78,.96,bevel=.045,corner=5)
    m.box('ventral.freight-elevator',(136,276,356,642),-1.18,-.88,bevel=.08,corner=18)
    for i,(x,y) in enumerate(((147,298),(344,298),(147,617),(344,617))):
        m.box(f'ventral.elevator-lock.{i}',(x-6,y-9,x+6,y+9),-1.23,-1.14,'brass',.02,2)
    for i,y in enumerate((205,341,476,642)):
        m.box(f'machinery.spine-access.{i}',(231,y,260,y+35),1.19,1.30,bevel=.03,corner=3)
    for side in (-1,1):
        for i,y in enumerate((686,731)):
            x=246+side*36
            m.cylinder(f'machinery.coolant.{side}.{i}',p(x,y,.65),p(x,y+30,.65),.17,'dark',12)


def build_second_wind(m):
    p=m.point
    m.slab('hull.workshop',[(123,233),(393,233),(438,289),(449,562),(411,660),
        (332,712),(229,713),(111,636),(81,532),(82,320)],-.95,1.0,.19)
    m.slab('ventral.reinforced-keel',[(176,245),(370,245),(405,290),(408,583),
        (325,687),(235,687),(130,611),(128,312)],-1.25,-.84,.13)
    for label,x0,x1 in [('port',145,211),('starboard',355,422)]:
        m.box(f'cradle.{label}.rail',(x0,58,x1,281),-.42,1.10,bevel=.13,corner=10)
        m.box(f'cradle.{label}.nose-cap',(x0+9,60,x1-9,143),1.06,1.25,bevel=.06,corner=5)
        m.box(f'cradle.{label}.wear-plate',(x0+6,151,x1-6,213),1.08,1.20,bevel=.03,corner=3)
        bolts(m,f'cradle.{label}.fastener',[(x0+10,74),(x1-10,74),(x0+10,235),(x1-10,235)],1.26,.06)
    # A loaded pallet spans the forward cradle. The gap behind it remains open.
    m.box('cargo.pallet-frame',(217,84,349,209),-.18,.65,'dark',.08,7)
    for i,(ya,yb) in enumerate(((90,144),(150,202))):
        m.box(f'cargo.pallet-module.{i}',(225,ya,341,yb),.61,1.08,bevel=.055,corner=4)
    for i,(x,y) in enumerate(((224,90),(341,90),(224,198),(341,198))):
        m.box(f'cargo.pallet-lock.{i}',(x-5,y-6,x+5,y+6),.99,1.20,'accent',.03,2)
    m.slab('bridge.glazing',[(78,215),(142,204),(183,229),(204,273),(203,325),
        (172,363),(114,368),(70,338),(49,287),(54,245)],.78,2.10,.22)
    m.box('bridge.roof-console',(83,208,162,231),2.04,2.23,bevel=.04,corner=8)
    for i,(x,y,h) in enumerate(((85,210,3.02),(100,212,2.82))):
        m.cylinder(f'bridge.antenna.{i}',p(x,y,2.13),p(x,y,h),.04,'dark',8)
    docking(m,'port',92,443,-1,.94,1.52,(1232,731,31))
    m.box('cargo.workshop.coaming',(211,346,347,581),.96,1.19,'accent',.06,12)
    m.box('cargo.workshop.lid',(221,357,338,570),1.15,1.34,bevel=.06,corner=10)
    bolts(m,'cargo.workshop.fastener',[(228,369),(331,369),(228,555),(331,555)],1.35,.075)
    for i,(x0,y0,x1,y1) in enumerate(((96,383,166,574),(367,402,432,505),(379,527,433,603))):
        m.box(f'armor.replacement-panel.{i}',(x0,y0,x1,y1),1.0,1.21,bevel=.05,corner=7)
    m.box('hull.starboard-loading-hatch',(427,367,454,535),-.50,.86,bevel=.08,corner=6)
    # Crane pivot and folded arm are separate closed solids, above the hull.
    m.lathe('crane.base',p(420,286,1.01),[0,0,1],[(0,.47),(.18,.51),(.40,.43)],24,'dark','dark')
    joints=[p(421,280,1.48),p(446,248,2.72),p(492,282,2.35),p(502,322,1.55)]
    for i,(a,b) in enumerate(zip(joints,joints[1:])):
        m.beam(f'crane.arm.{i}',a,b,.38 if i<2 else .28,.40,'accent')
        m.cylinder(f'crane.hydraulic.{i}',a+[-.15,0,-.18],b+[-.15,0,-.18],.067,'silver',12)
    for i,q in enumerate(joints):
        m.cylinder(f'crane.pivot.{i}',q+[-.28,0,0],q+[.28,0,0],.22,'dark',16)
        m.cylinder(f'crane.pivot-cap.{i}',q+[.28,0,0],q+[.33,0,0],.13,'brass',12)
    wrist=joints[-1]
    m.cylinder('crane.wrist',wrist,wrist+[0,-.38,-.08],.17,'dark',16)
    for side in (-1,1):
        a=wrist+np.array([side*.10,-.30,-.05])
        b=wrist+np.array([side*.29,-.68,-.17])
        c=wrist+np.array([side*.17,-.98,-.23])
        m.beam(f'crane.jaw.{side}.root',a,b,.13,.18,'dark')
        m.beam(f'crane.jaw.{side}.tip',b,c,.13,.18,'silver')
    m.socket('socket.utility.crane',wrist+[0,-1.02,-.23],[0,-1,0],'salvage-tool-grip')
    for label,x in [('port',143),('starboard',419)]:
        m.box(f'hull.{label}.engine-mount',(x-32,592,x+31,674),-.55,.72,'dark',.07,7)
        runner_engine(m,label,x,646,811,1.60,.03,(1235 if label=='port' else 1595,474,53),square=label=='starboard',bands=label=='port')
        m.box(f'engine.{label}.service-panel',(x-18,669,x+18,718),1.52,1.70,bevel=.04,corner=4)
        m.cylinder(f'machinery.{label}.feed-pipe',p(x+20,571,.47),p(x+20,650,.47),.14,'dark',12)
    for i,y in enumerate((277,394,519)):
        m.box(f'ventral.cargo-hatch.{i}',(236,y,333,y+88),-1.36,-1.22,bevel=.04,corner=8)
    for side in (-1,1):
        x=278+side*112
        for i,(ya,yb) in enumerate(((290,400),(423,566))):
            m.beam(f'ventral.brace.{side}.{i}',p(x,ya,-1.18),p(278+side*48,yb,-1.18),.14,.17,'dark')
    for i,(x,y) in enumerate(((178,395),(176,500),(379,367),(365,585))):
        m.cylinder(f'machinery.reservoir.{i}',p(x,y,.70),p(x,y+39,.70),.18,'brass',12)


def build_small_fortune(m):
    p=m.point
    loft(m,'hull.spade-body',[(49,262,25,-.14,.70),(91,262,61,-.48,1.14),
        (151,262,100,-.73,1.26),(240,262,148,-.87,1.10),
        (327,262,155,-.94,1.05),(407,262,134,-.83,.95),
        (464,262,91,-.68,.73),(509,262,48,-.59,.66)])
    m.slab('bridge.glazing',[(216,97),(296,97),(339,131),(363,169),(342,190),
        (180,190),(146,169),(170,128)],1.05,1.82,.19)
    m.box('bridge.roof-cap',(232,68,291,116),.99,1.20,bevel=.06,corner=9)
    for i,(x,y) in enumerate([(x,y) for x in (148,291) for y in (250,345)]):
        m.box(f'cargo.bay.{i+1}.coaming',(x-5,y-5,x+100,y+88),1.02,1.19,'dark',.04,6)
        m.box(f'cargo.bay.{i+1}.lid',(x,y,x+94,y+82),1.15,1.34,bevel=.06,corner=9)
        m.box(f'cargo.bay.{i+1}.latch',(x-3,y+17,x+4,y+46),1.29,1.43,'brass',.02,2)
        bolts(m,f'cargo.bay.{i+1}.fastener',[(x+8,y+8),(x+86,y+8),(x+8,y+74),(x+86,y+74)],1.35)
    m.box('armor.cream-spine',(246,206,279,475),1.03,1.19,bevel=.04,corner=4)
    m.slab('hull.service-tail',[(224,450),(299,450),(307,528),(288,619),(267,636),
        (237,618),(215,529)],-.52,.74,.12)
    m.box('machinery.tail-spine',(239,476,284,568),.72,.95,bevel=.06,corner=5)
    for side,label in ((-1,'port'),(1,'starboard')):
        mirror=lambda x:262+side*(x-262)
        # Two separate swept supports leave open space beside the narrow tail.
        for i,outline in enumerate([
            [(389,379),(414,391),(430,489),(408,510),(386,444)],
            [(294,474),(316,467),(405,527),(407,552),(384,556),(304,506)]]):
            m.slab(f'hull.{label}.pylon.{i}',[(mirror(x),y) for x,y in outline],-.38,.47,.09)
        m.beam(f'hull.{label}.pylon-brace',p(mirror(307),493,.48),p(mirror(398),545,.48),.17,.13,'brass')
        m.cylinder(f'machinery.{label}.conduit',p(mirror(407),371,.19),p(mirror(417),473,.19),.13,'dark',12)
        x=mirror(420)
        runner_engine(m,label,x,469,807,1.64,.06,(1159 if side<0 else 1585,480,61))
        m.box(f'engine.{label}.dorsal-shield',(x-25,511,x+25,666),1.60,1.83,bevel=.065,corner=6)
        for j,sy in enumerate((518,617)):
            bolts(m,f'engine.{label}.shield-bolt.{j}',[(x-18,sy),(x+18,sy)],1.84,.06,'brass')
    docking(m,'port',82,321,-1,.73,.69,(1197,699,24))
    m.box('hull.starboard-loading-lock',(402,282,459,352),-.51,.92,bevel=.10,corner=7)
    # The circular plan-view fitting is a dorsal service hatch above the collar.
    m.lathe('machinery.port-service-hatch',p(103,320,.88),[0,0,1],
        [(0,.74),(.14,.81),(.28,.77),(.35,.64)],24,'auto','brass',cap=(103,320,30))
    m.box('ventral.belly-cargo-door',(171,258,355,447),-1.11,-.88,bevel=.07,corner=17)
    m.box('ventral.service-tail-hatch',(237,489,285,567),-.69,-.49,bevel=.04,corner=6)
    for i,(x,y) in enumerate(((183,280),(344,280),(183,420),(344,420))):
        m.box(f'ventral.door-lock.{i}',(x-5,y-9,x+5,y+9),-1.18,-1.07,'brass',.02,2)
    for i,y in enumerate((208,453,585)):
        m.box(f'machinery.spine-access.{i}',(249,y,273,y+30),.81 if y>500 else 1.18,1.01 if y>500 else 1.34,bevel=.03,corner=3)


BUILDERS={'clean-slate':build_clean_slate,'second-wind':build_second_wind,'small-fortune':build_small_fortune}


def build_one(slug):
    out=ROOT.parent/(slug+'-textured-v1')
    out.mkdir(parents=True,exist_ok=True)
    model=Trader(out,copy.deepcopy(SPECS[slug]))
    model.prepare_textures()
    BUILDERS[slug](model)
    model.export()
    return out


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('ships',nargs='*')
    args=parser.parse_args()
    for slug in args.ships or SPECS:
        if slug not in SPECS:parser.error('Unknown ship: '+slug)
        build_one(slug)
