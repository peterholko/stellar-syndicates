"""Hand-traced, closed, low-polygon reconstructions of the five-view sheets.

Coordinates are registered to the TOP illustration (pixels); heights are model
units. Hull undersides use the independently registered BOTTOM illustration.
All engine mouths, barrels and open grapple cages are geometry, not cards.
"""
from geometry import *


def tube(name, centers, radius=.09, material=graphite, closed=False,
         binormal=(0, 0, 1), segments=6, mode='auto'):
    """A low-sided pipe following a planar path; loops have welded positions."""
    centers = np.asarray(centers, float)
    vertices = []
    count = len(centers)
    for i, center in enumerate(centers):
        before = centers[(i-1) % count] if closed or i else centers[i]
        after = centers[(i+1) % count] if closed or i < count-1 else centers[i]
        tangent = unit(after-before)
        normal = unit(np.cross(np.asarray(binormal, float), tangent))
        binorm = unit(np.cross(tangent, normal))
        vertices.extend(center+radius*(math.cos(j*2*math.pi/segments)*normal+
                                      math.sin(j*2*math.pi/segments)*binorm)
                        for j in range(segments))
    faces = []
    for i in range(count if closed else count-1):
        a, b = i*segments, ((i+1) % count)*segments
        faces.extend((a+j, a+(j+1) % segments, b+(j+1) % segments, b+j)
                     for j in range(segments))
    if not closed:
        faces.extend([tuple(reversed(range(segments))),
                      tuple(range((count-1)*segments, count*segments))])
    return mesh(name, vertices, faces, mat=material, mode=mode, smooth=True)


def engine(index, engine_spec, length=170):
    """Closed motor, hollow nozzle lip and recessed textured/emissive chamber."""
    x, y, r, z = (engine_spec[k] for k in ['x', 'y', 'r', 'z'])
    name = f'engine.{index}'
    radial(name+'.motor', x, [(y-length, r*.73), (y-length+32, r*.87),
                             (y-75, r*.93), (y-39, r*.97)], z, segments=12)
    # Closed annular profile: the empty center is genuinely open geometry.
    radial(name+'.nozzle', x, [(y-42, r*.97), (y-15, r*.96), (y, r*.83),
                             (y, r*.69), (y-24, r*.61), (y-42, r*.75),
                             (y-42, r*.97)], z, cap=False, segments=12)
    ring(name+'.aft.collar', x, y-20, z, r*.998, r*.94, .11, graphite)
    ring(name+'.forward.collar', x, y-86, z, r*.97, r*.90, .12, graphite)
    bx, by, br = engine_spec['back']
    center_x = (x-CX)/S
    def exhaust_uv(v):
        return ((bx+(v[0]-center_x)/r*br)/2048,
                (by-(v[2]-z)/r*br)/2048)
    radial(name+'.recessed.combustor', x, [(y-25, r*.61), (y-23, r*.61)], z,
           mode=lambda normal, center: (atlas, exhaust_uv), segments=12)
    # Two narrow exterior stiffeners, kept inside the reference silhouette.
    for k, angle in enumerate([math.pi*.19, math.pi*.81]):
        px = x+math.cos(angle)*r*.98*S
        pz = z+math.sin(angle)*r*.98
        beam(name+f'.brace.{k}', point(px, y-111, pz), point(px, y-24, pz),
             .065, .065, graphite)
    sockets.append((f'engine.{index}.exhaust', point(x, y+1, z)))


def gun(name, x, nose, rear, z, radius=.15):
    radial(name+'.barrel', x, [(nose, radius*.73), (nose+7, radius*.87),
                              (nose+29, radius*.87), (nose+34, radius),
                              (rear-13, radius), (rear, radius*1.2)], z,
           mode=lambda n, c: (dark if abs(n[1])>.7 else atlas, top_uv), segments=8)
    ring(name+'.muzzle', x, nose, z, radius, radius*.68, .085, brass)
    radial(name+'.bore', x, [(nose+1, radius*.68), (nose+3, radius*.68)], z,
           mat=dark, segments=8)
    sockets.append((name+'.muzzle.socket', point(x, nose-1, z)))


def privateer():
    loft('armored.pressure.hull', [
        (94, 10, 1.00, 1.24, 1.39), (145, 31, .78, 1.48, 1.75),
        (255, 71, .53, 1.67, 2.12), (305, 77, .40, 1.78, 2.09),
        (446, 79, .36, 1.93, 2.30), (538, 77, .23, 1.82, 2.20),
        (652, 80, .19, 1.88, 2.29), (679, 76, .24, 1.69, 2.07),
        (744, 65, .33, 1.29, 1.50), (797, 47, .42, 1.04, 1.20)])
    # The wing tips and replacement armor deliberately retain the sheet's
    # asymmetry instead of reflecting the starboard orange/yellow panels.
    wing_height = lambda x, y: 1.19+.42*clamp(1-abs(x-CX)/255)
    slab('port.swept.wing', [(26, 619), (129, 516), (188, 499),
                           (193, 571), (180, 674), (36, 704), (25, 692)],
         wing_height, .86, bevel=.075)
    slab('starboard.swept.wing', [(359, 503), (417, 514), (514, 619),
                                (515, 690), (503, 704), (370, 672)],
         wing_height, .86, bevel=.075)
    slab('port.engine.haunch', [(149, 594), (218, 565), (227, 701),
                               (201, 759), (149, 769), (114, 730), (116, 671)],
         1.23, .47, bevel=.10)
    slab('starboard.engine.haunch', [(332, 566), (411, 594), (442, 661),
                                    (443, 728), (402, 769), (350, 757)],
         1.28, .47, bevel=.10)
    def cockpit_side(v):
        px = 1096+(CY-v[1]*S-309)/(466-309)*110
        return registered_uv('side', px, 870-v[2]*30)
    def cockpit_hood(v):
        return ((1257-v[0]*36)/2048, (277.7-v[2]*34.8)/2048)
    def cockpit_mapping(n, c):
        axis = int(np.argmax(np.abs(n)))
        if axis == 0:
            return atlas, cockpit_side
        if n[1] > .05:
            return atlas, cockpit_hood
        return (ventral, bottom_uv) if n[2] < -.5 else (atlas, top_uv)
    loft('raised.cockpit', [(309, 14, 1.86, 2.21, 2.45),
                           (328, 31, 1.91, 2.52, 3.05),
                           (378, 39, 1.95, 2.76, 3.47),
                           (434, 33, 2.00, 2.70, 3.29),
                           (466, 18, 1.99, 2.35, 2.63)], mode=cockpit_mapping)
    for label, x in [('port', 158), ('starboard', 386)]:
        bevel_box(label+'.gun.mount', (x-31, 438, x+31, 552),
                  1.26, 2.49, bevel=.15, corner=.27)
        bevel_box(label+'.gun.top.plate', (x-23, 444, x+24, 529),
                  2.47, 2.57, bevel=.026)
        for j, dx in enumerate([-9, 10]):
            gun(label+f'.gun.{j}', x+dx, 350, 442, 2.33, .16)
        # Intakes have depth, with a real shadowed floor and three broad vanes.
        bevel_box(label+'.intake', (x-24, 529, x+24, 553), 1.74, 2.12,
                  mat=dark, bevel=.05)
        for j in range(3):
            beam(label+f'.intake.vane.{j}', point(x-19, 533+j*6, 2.125),
                 point(x+19, 533+j*6, 2.125), .045, .045, graphite)
    bevel_box('aft.dorsal.plate', (240, 532, 300, 630), 2.17, 2.35, bevel=.06)
    # A closed stepped keel gives the bottom view real relief.
    slab('ventral.keel', [(248, 237), (298, 237), (307, 472), (315, 663),
                         (302, 779), (247, 779), (232, 662), (241, 470)],
         .66, lambda x,y: interpolate(y, [(237,.49),(400,.08),(550,-.035),(680,.08),(779,.32)]), bevel=.055)
    bevel_box('ventral.access.hatch', (249, 480, 296, 578), -.10, .18,
              bevel=.018)
    for label, x in [('port', 217), ('starboard', 332)]:
        tube(label+'.cockpit.conduit', [point(x, py, pz) for py, pz in
             [(290, 1.64), (372, 1.9), (486, 1.7), (512, 1.2)]],
             radius=.068, material=brass, binormal=(1, 0, 0))
    for i, spec in enumerate(SPEC['engines']):
        engine(i, spec, length=178 if i != 1 else 190)


def corsair():
    loft('needle.pressure.hull', [
        (86, 2.3, .45, .55, .63), (114, 13, .32, .66, .84),
        (221, 25, .21, .79, 1.00), (330, 37, .16, .89, 1.21),
        (381, 44, .14, .98, 1.32), (485, 42, .09, 1.07, 1.29),
        (566, 65, .07, 1.04, 1.28), (702, 71, .06, .98, 1.25),
        (793, 77, .08, .87, 1.07), (824, 64, .09, .75, .91)])
    high = lambda x, y: .40+.59*clamp(1-abs(x-CX)/266)
    low = lambda x, y: high(x, y)-.25
    slab('port.hooked.wing', [(196, 463), (196, 528), (231, 539),
                            (229, 731), (209, 759), (192, 742),
                            (172, 747), (106, 768), (65, 788),
                            (43, 812), (29, 847), (22, 879),
                            (18, 858), (25, 810), (41, 750),
                            (72, 671), (121, 580)], high, low, bevel=.052)
    slab('starboard.hooked.wing', [(368, 463), (417, 543), (465, 636),
                                  (499, 722), (512, 781), (512, 824),
                                  (497, 799), (476, 778), (424, 748),
                                  (402, 737), (383, 735), (361, 747),
                                  (331, 754), (328, 546), (367, 524)],
         high, low, bevel=.052)
    loft('dorsal.spine', [(253, 12, .89, 1.08, 1.27),
                         (306, 18, 1.00, 1.22, 1.55),
                         (365, 17, 1.08, 1.22, 1.46),
                         (505, 22, 1.08, 1.29, 1.49),
                         (625, 27, 1.07, 1.24, 1.38),
                         (738, 22, .94, 1.14, 1.30),
                         (782, 16, .85, 1.06, 1.19)])
    for label, x in [('port', 194), ('starboard', 370)]:
        bevel_box(label+'.gun.shoe', (x-9, 524, x+10, 581),
                  .79, 1.15, bevel=.04)
        radial(label+'.cannon.receiver', x,
               [(526, .16), (537, .27), (558, .27), (574, .19)],
               1.22, segments=8)
        gun(label+'.cannon', x, 492, 540, 1.22, .12)
    for side in [-1, 1]:
        tube(f'wing.root.pipe.{side}', [point(CX+side*dx, py, z)
             for dx, py, z in [(30, 409, .79), (40, 446, .94),
                               (53, 467, .96), (62, 496, .78), (60, 516, .65),
                               (40, 546, .61)]],
             radius=.105, material=brass)
    slab('ventral.service.keel', [(260, 255), (300, 255), (303, 436),
                                 (310, 651), (301, 778), (262, 778),
                                 (249, 651), (258, 433)], .28, -.045,
         bevel=.04)
    bevel_box('ventral.reactor.hatch', (264, 464, 297, 542), -.12, -.035,
              bevel=.017)
    for i, spec in enumerate(SPEC['engines']):
        engine(i, spec, length=99)


def grapple(label, x, y, transverse_radius):
    """Two perpendicular elliptical frames form one open boarding cage."""
    z, length = 1.33, 3.00
    rx = transverse_radius
    angles = np.arange(18)*2*math.pi/18
    xy = [point(x+rx*S*math.cos(a), y+length*S*math.sin(a), z) for a in angles]
    yz = [point(x, y+length*S*math.sin(a), z+1.18*math.cos(a)) for a in angles]
    tube(label+'.horizontal.frame', xy, .135, atlas, True, segments=6)
    tube(label+'.vertical.frame', yz, .125, atlas, True, binormal=(1, 0, 0),
         segments=6, mode=lambda n, c: (atlas, side_uv))
    # Hub axis points sideways. It joins both open frames to the hull arm.
    def hub_uv(v):
        return ((x+(v[2]-z)*S)/2048, (CY-v[1]*S)/2048)
    def hub_mapping(normal, center):
        return (atlas, hub_uv) if abs(normal[0])>.5 else hull_mapping(normal, center)
    transverse_shell(label+'.hub', [(x-12, .83), (x-9, 1), (x+9, 1), (x+12, .83)],
                     y, .47, .47, z, mode=hub_mapping, segments=12)
    for i, a in enumerate([0, math.pi]):
        beam(label+f'.horizontal.spoke.{i}', point(x, y, z),
             point(x+rx*S*math.cos(a), y, z), .14, .16, silver)
        beam(label+f'.vertical.spoke.{i}', point(x, y, z),
             point(x, y, z+1.18*math.cos(a)), .14, .16, graphite)
    inner_x = CX+(x-CX)*.63
    beam(label+'.hull.arm', point(inner_x, y+26, z), point(x, y, z),
         .30, .34, graphite)
    beam(label+'.hydraulic.ram', point(inner_x, y+22, z+.18),
         point(x, y, z+.18), .12, .13, silver)
    # Small collision jaws at the two axial ends, with texture from the sheet.
    for i, py in enumerate([y-118, y+118]):
        bevel_box(label+f'.jaw.{i}', (x-11, py-7, x+11, py+7),
                  z-.24, z+.24, bevel=.045, corner=.07)
    sockets.append((label+'.capture', point(x, y, z)))


def boarding():
    loft('wedge.pressure.hull', [
        (96, 5, 1.69, 1.78, 1.86), (157, 31, 1.22, 1.58, 1.90),
        (265, 91, .48, 1.66, 2.10), (358, 113, .36, 1.83, 2.29),
        (449, 145, .28, 1.97, 2.47), (553, 167, .20, 1.86, 2.43),
        (663, 190, .17, 1.69, 2.26), (717, 174, .23, 1.52, 2.08),
        (771, 149, .27, 1.28, 1.74), (799, 128, .32, 1.14, 1.48)])
    # Separate battered plate rows give the salvage hull its layered silhouette.
    for side in [-1, 1]:
        slab(f'fore.armor.{side}', [(239, 206), (280, 145), (283, 203),
                                   (265, 243), (214, 279)],
             1.78, 1.57, side=side, bevel=.04)
        slab(f'shoulder.armor.{side}', [(207, 287), (257, 276), (251, 368),
                                       (175, 387)],
             1.99, 1.77, side=side, bevel=.04)
        slab(f'mid.armor.{side}', [(168, 397), (253, 391), (254, 481),
                                  (144, 468)],
             2.02, 1.77, side=side, bevel=.04)
        slab(f'boarding.arm.armor.{side}', [(130, 550), (191, 575),
                                           (206, 661), (177, 716), (95, 734)],
             1.74, 1.38, side=side, bevel=.05)
    loft('dorsal.weapon.rail', [(152, 8, 1.59, 1.82, 2.02),
                               (215, 11, 1.78, 2.05, 2.20),
                               (268, 23, 1.98, 2.35, 2.66),
                               (351, 22, 2.12, 2.55, 2.87),
                               (404, 27, 2.05, 2.66, 2.93)])
    # Offset raised bridge, traced to the reference rather than centered.
    slab('offset.bridge.base', [(359, 380), (410, 385), (438, 413),
                               (452, 489), (431, 540), (398, 561),
                               (346, 541), (334, 472), (336, 416)],
         2.92, 2.19, bevel=.10)
    def bridge_side(v):
        px = 1205+(CY-v[1]*S-404)/(514-404)*126
        return registered_uv('side', px, 898-v[2]*43)
    def bridge_mapping(n, c):
        if abs(n[0]) > max(abs(n[1]), abs(n[2])):
            return atlas, bridge_side
        return (ventral, bottom_uv) if n[2] < -.5 else (atlas, top_uv)
    loft('offset.bridge.canopy', [(404, 8, 2.84, 3.05, 3.24),
                                 (428, 29, 2.84, 3.28, 3.83),
                                 (477, 31, 2.86, 3.26, 3.77),
                                 (514, 15, 2.80, 3.10, 3.43)], center=388, mode=bridge_mapping)
    # A few actual canopy crossbars strengthen the silhouette at close range.
    for j, py in enumerate([429, 477]):
        beam(f'bridge.crossbar.{j}', point(368, py, 3.84), point(408, py, 3.84),
             .065, .065, brass)
    for j in range(4):
        bevel_box(f'amber.rail.vent.{j}', (288+j*8, 383, 292+j*8, 397),
                  2.937, 2.962, bevel=.005, mat=amber)
    slab('ventral.boarding.keel', [(281, 251), (322, 251), (332, 401),
                                  (336, 559), (333, 704), (276, 704),
                                  (267, 564), (272, 402)], .56,
         lambda x,y: interpolate(y, [(251,.45),(400,.08),(560,-.015),(704,.13)]),
         bevel=.045)
    bevel_box('ventral.boarding.airlock', (276, 412, 330, 556), -.12, .23,
              bevel=.027, corner=.15)
    for side in [-1, 1]:
        tube(f'ventral.hydraulic.{side}', [point(CX+side*dx, py, z)
             for dx, py, z in [(39, 285, .46), (44, 402, .31),
                               (44, 556, .28), (55, 585, .28), (67, 706, .51)]],
             radius=.09, material=brass)
    for i, spec in enumerate(SPEC['engines']):
        engine(i, spec, length=168)
    loft('center.engine.armored.cowl', [(700, 16, 1.67, 2.23, 2.49),
                                       (770, 16, 1.79, 2.21, 2.41),
                                       (832, 27, 1.45, 1.96, 2.18),
                                       (868, 26, 1.13, 1.61, 1.82)])
    grapple('port.grapple', 61, 613, 1.04)
    grapple('starboard.grapple', 500, 613, .79)


DESIGNS = {'privateer-raider': privateer, 'pirate-corsair': corsair,
           'pirate-boarding-raider': boarding}
