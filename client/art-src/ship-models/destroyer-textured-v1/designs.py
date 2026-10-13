"""Hand-traced, editable contours from the nine generated TOP drawings.

Every design uses closed solids and dedicated underside UVs. SIDE and end
drawings supply depth and finish; no reference image is used as a flat card.
"""

def build(ship,namespace):
    globals().update(namespace)
    globals()['make_'+ship.replace('-','_')]()

def make_destroyer():
    loft('hull | continuous aft pressure hull',[(296,22,.7,1.3,1.6),(346,67,.5,1.7,2),
         (440,96,.3,1.7,2.1),(530,87,.3,1.6,1.95),(589,111,.3,1.6,1.85),
         (680,119,.35,1.6,1.95),(741,63,.5,1.5,1.8),(774,39,.6,1.3,1.5)])
    rail=[(293,67),(302,68),(319,115),(348,165),(346,261),(360,317),
          (354,344),(330,366),(335,420),(311,436),(298,338),(292,206)]
    shoulders=[(323,546),(353,568),(393,613),(428,651),(428,690),(373,718),(318,711),(311,612)]
    for side,label in [(-1,'port'),(1,'starboard')]:
        slab('armor.'+label+' | long fork rail',rail,
             lambda x,y:interpolate(y,[(66,1.2),(165,1.8),(338,1.95),(440,1.8)]),
             lambda x,y:interpolate(y,[(66,.85),(165,.5),(338,.36),(440,.38)]),side,bevel=.055)
        slab('armor.'+label+' | flared aft shoulder',shoulders,2.02,.36,side,bevel=.10)
        slab('keel.'+label+' | underside shoulder',[(326,563),(387,617),(420,653),(420,688),(368,711),(327,698)],.46,.25,side,bevel=.04)
        turret(label,1,CX+side*110,632,2.06,r=.41,barrel_length=34)
        navigation(CX+side*81,460,2.04,label)
    loft('spine | dorsal service spine',[(397,19,1.75,2,2.12),(531,22,1.72,2.0,2.2),
         (621,26,1.6,1.9,2.15),(711,35,1.65,1.95,2.2)])
    loft('keel | ventral service spine',[(310,15,.5,.7,.9),(446,23,.17,.4,.53),(628,25,.15,.37,.48),(754,22,.4,.6,.8)])
    bridge((247,293,292,409),1.85,.82)
    antenna(270,295,2.45,.87)
    for name,x,r,end,uv,ruv in [('port',178,.92,789,(1128,512),38),('center',270,1.25,823,(1280,509),48),('starboard',362,.92,789,(1432,512),38)]:
        engine(name,x,694,end,r,1.36,uv,ruv)

def make_battleship():
    loft('hull | deep armored pressure hull',[(71,47,.7,1.7,2.1),(142,94,.35,2.1,2.55),
         (249,102,.12,2.3,2.65),(372,109,.02,2.2,2.55),(504,107,.03,2.18,2.55),
         (628,118,.13,2.1,2.46),(723,96,.25,1.95,2.3),(782,58,.45,1.6,1.98)])
    cheek=[(299,70),(314,69),(365,115),(371,174),(362,220),(374,246),(358,295),(331,345),(312,335)]
    battery=[(372,304),(439,306),(468,325),(472,504),(452,526),(382,533),(373,496)]
    for side,label in [(-1,'port'),(1,'starboard')]:
        slab('armor.'+label+' | bow cheek',cheek,2.62,.5,side,bevel=.10)
        slab('armor.'+label+' | broadside sponson',battery,2.22,.30,side,bevel=.10)
        box('armor.'+label+' | rear flank',
            (min(CX+side*92,CX+side*145),594,max(CX+side*92,CX+side*145),751),.46,2.14,bevel=.13)
        for i,py in enumerate((373,448),1):
            turret(label,i,CX+side*171,py,2.26,.53,62,math.pi/2 if side<0 else -math.pi/2)
        navigation(CX+side*69,467,2.66,label)
    bridge((233,87,284,197),2.50,.93)
    antenna(260,96,3.25,.68)
    for i,py in enumerate((286,425,565),1):turret('center',i,260,py,2.66,.79,66)
    for i,py in enumerate((263,397,525,651),1):
        box(f'keel.panel.{i} | maintenance cover',(235,py-38,285,py+38),-.05,.2,bevel=.04)
    loft('spine | centerline stern fairing',[(607,30,2.0,2.43,2.7),(664,27,1.9,2.41,2.61),(710,21,1.7,2.1,2.35)])
    # A genuine 3-by-2 grid; top-view lower-row drives are occluded.
    for row,z in enumerate((.81,2.33),1):
        for i,(px,u) in enumerate(((201,1300),(260,1357),(319,1415)),1):
            engine(f'{row}.{i}',px,693,817,.99,z,(u,572 if row==1 else 516),23,upper=row==2)

def make_dreadnought():
    # The top drawing contains nine dorsal mounts and eight broadside mounts.
    for side,label in [(-1,'port'),(1,'starboard')]:
        cx=CX+side*123
        loft('hull.'+label+' | outer pressure hull',[(106,27,.5,1.55,1.9),(161,47,.25,1.94,2.3),
             (214,66,.1,2.2,2.57),(296,58,.1,2.14,2.48),(373,70,.08,2.13,2.48),
             (451,61,.04,2.17,2.52),(540,64,.08,2.18,2.5),(620,62,.15,2.07,2.39),
             (702,73,.26,1.9,2.24),(745,53,.45,1.6,1.98)],center=cx)
        flank=[(426,339),(461,362),(476,391),(475,433),(449,446),(423,426)]
        slab('armor.'+label+' | side radiator shroud',flank,2.1,.5,side,bevel=.11)
        aft=[(418,576),(453,598),(477,617),(465,651),(477,673),(457,713),(425,716),(407,662)]
        slab('armor.'+label+' | stern shoulder',aft,2.16,.4,side,bevel=.11)
        for i,py in enumerate((309,428,554),1):
            x0,x1=sorted((CX+side*27,CX+side*82))
            box(f'structure.{label}.{i} | transverse hull bridge',(x0,py-20,x1,py+20),.64,1.80,bevel=.08)
            for y in (py-16,py+16):
                beam(f'structure.{label}.{i} | edge strap',point(x0,y,1.87),point(x1,y,1.87),.10,.09,brass)
        for i,py in enumerate((114,235,469),1):turret(label,i,cx,py,1.94 if i==1 else 2.59,.40,38)
        for i,py in enumerate((265,329,491,556),4):
            turret(label,i,CX+side*199,py,1.55,.39,25,math.pi/2 if side<0 else -math.pi/2)
        navigation(CX+side*170,466,2.42,label)
        for i,(off,r) in enumerate(((99,.94),(149,.66)),1):
            px=CX+side*off
            ux={(-1,1):1202,(-1,2):1138,(1,1):1537,(1,2):1600}[(side,i)]
            engine(f'{label}.{i}',px,682,793 if i==1 else 775,r,1.27,(ux,546),29 if i==1 else 24)
    loft('hull.center | narrow command spine',[(158,15,.55,1.7,2),(209,34,.31,2,2.25),
         (281,28,.23,1.92,2.16),(377,22,.18,1.87,2.14),(467,31,.16,2,2.27),
         (576,27,.21,1.93,2.2),(685,43,.29,1.83,2.18),(748,37,.38,1.64,1.96)])
    bridge((225,173,284,272),2.20,.82)
    for i,py in enumerate((321,490,559),1):turret('center',i,254,py,2.25,.27,27)
    antenna(254,165,2.55,.83)
    for py in (328,430,560,651):box('keel.center | ventral service hatch',(239,py-30,269,py+30),.09,.28,bevel=.028)
    engine('center',254,689,811,1.22,1.41,(1369,547),35)

def make_scout():
    loft('hull | slender reconnaissance fuselage',[(83,9,.72,1.18,1.3),(153,26,.37,1.4,1.65),
         (253,43,.17,1.62,1.93),(313,58,.06,1.63,1.91),(387,59,.0,1.59,1.82),
         (523,64,.02,1.57,1.82),(615,69,.09,1.58,1.91),(696,60,.25,1.54,1.77),
         (771,11,.58,1.02,1.25)])
    radial('sensor | forward needle',306,[(17,.035),(44,.066),(105,.067),(115,.055)],1.08,mat=graphite,segments=24)
    bridge((279,151,336,267),1.54,.72)
    zbody('sensor | dorsal scanner bezel',306,403,[(.62,1.79),(.73,1.88),(.64,2.05),(.53,2.12)],brass,48)
    zbody('sensor | scanner lens',306,403,[(.51,2.10),(.50,2.17)],atlas,48,mode=lambda n,c:(atlas,top_uv) if n[2]>.1 else (graphite,top_uv))
    for side,label in [(-1,'port'),(1,'starboard')]:
        px=CX+side*115
        beam('structure.'+label+' | swept outrigger',point(CX+side*69,673,.91),point(px,612,.91),.28,.29,graphite)
        beam('structure.'+label+' | outrigger upper rail',point(CX+side*69,673,1.09),point(px,612,1.09),.14,.06,silver)
        zbody('structure.'+label+' | bearing',px,612,[(.32,.65),(.32,1.27)],graphite,32)
        zbody('service.'+label+' | bearing lid',px,612,[(.26,1.27),(.26,1.31)],brass,32)
        radial('sensor.'+label+' | remote sensor pod',px,[(492,.29),(508,.39),(586,.39),(610,.28)],1.12,segments=32)
        radial('sensor.'+label+' | long probe',px,[(446,.035),(455,.052),(496,.052)],1.12,mat=silver,segments=24)
        navigation(px,540,1.52,label)
        engine(label,CX+side*48,672,814,.79,1.08,(1249 if side<0 else 1395,519),34)
    loft('keel | ventral instrumentation tray',[(199,13,.11,.3,.45),(385,23,-.06,.18,.30),
         (563,25,-.03,.17,.29),(730,15,.20,.41,.51)])

def make_colony_ship():
    loft('spine | inhabited central keel',[(67,8,.9,1.65,1.95),(113,23,.58,1.86,2.14),
         (253,28,.4,1.91,2.23),(434,28,.36,1.97,2.3),(658,28,.31,1.96,2.29),
         (754,25,.53,1.7,2),(808,13,.6,1.4,1.64)])
    habitat('forward',266,208,111,97,1.69,2.34,1.53)
    for label,x in [('port',161),('starboard',373)]:
        for i,y in enumerate((366,626),1):
            habitat(f'{label}.{i}',x,y,74,76,1.55,1.62,1.4)
            out=55 if label=='port' else 478
            beam(f'structure.{label}.{i} | outer life-support connection',point(x,y,1.48),point(out,y,1.48),.30,.40,graphite)
            beam(f'structure.{label}.{i} | habitat coupling',point(266,y,1.48),point(x,y,1.48),.43,.48,graphite)
            box(f'cargo.{label}.{i} | external life support',(out-19,y-26,out+19,y+26),.81,2.18,bevel=.12)
            navigation(out,y,2.19,f'{label}.{i}')
    # This is a real elliptical tube with an open center, lying in the ship plane.
    ellipse_ring('habitat.ring | glazed horizontal habitat torus',266,497,4.67,.94,.39,1.55,around=96,cross=16)
    for x in (160,371):beam('structure | habitat ring radial mount',point(x,456,1.49),point(x,534,1.49),.24,.24,brass)
    for x in (128,408):
        box('cargo | forward life support',(x-18,203,x+18,259),1.00,2.3,bevel=.12)
        beam('structure | forward life-support connection',point(x,231,1.60),point(266,231,1.60),.30,.40,graphite)
    for side,label in [(-1,'port'),(1,'starboard')]:
        px=CX+side*49
        engine(label,px,701,836,.96,1.35,(1295 if side<0 else 1434,522),33)
        box('armor.'+label+' | drive root',(min(px-24,CX),697,max(px+24,CX),762),.45,2.14,bevel=.10)
    antenna(267,68,2.08,.76)
    sockets.append(('socket.bridge',point(266,105,2.20)))

def make_troop_transport():
    loft('hull | armored troop transport core',[(77,24,.7,1.8,2.3),(119,59,.34,2.1,2.65),
         (191,95,.1,2.12,2.54),(267,115,-.01,2.03,2.46),(413,116,-.02,2.05,2.48),
         (554,101,.07,2.08,2.47),(629,77,.18,1.98,2.35),(697,51,.29,1.82,2.21)])
    bridge((203,102,335,221),2.08,.9)
    box('spine | central boarding hatch',(224,342,318,580),2.40,2.6,bevel=.08)
    zbody('service | docking hatch ring',270,536,[(.51,2.57),(.63,2.64),(.53,2.71)],brass,40)
    zbody('service | docking hatch lid',270,536,[(.46,2.70),(.46,2.73)],graphite,40)
    for label,x,side in [('port',86,-1),('starboard',454,1)]:
        for i,(y0,y1) in enumerate(((217,412),(436,636)),1):
            cargo_box(label,i,(x-46,y0,x+46,y1),.30,2.17)
            for py in (y0+57,y1-33):
                beam(f'structure.{label}.{i} | armored docking tunnel',point(CX+side*103,py,1.21),point(x-side*40,py,1.21),.48,.64,graphite)
                ring(f'structure.{label}.{i} | docking collar',CX+side*130,py,1.21,.23,.16,.12,brass)
            navigation(x,y0+22,2.29,f'{label}.{i}')
        engine(label,CX+side*78,597,787,1.37,1.55,(1304 if side<0 else 1434,502),39)
    for py in (259,419,555):box('keel | armored ventral access',(231,py-40,311,py+40),-.13,.17,bevel=.05)

def make_construction_ship():
    loft('hull | aft engineering pressure hull',[(404,40,.53,1.79,2.11),(463,98,.25,2.1,2.4),
         (542,100,.11,2.11,2.43),(650,100,.07,2.01,2.34),(737,87,.24,1.84,2.11),(790,28,.5,1.52,1.79)])
    box('structure | transverse crane base',(115,380,420,447),.49,1.81,bevel=.09)
    bridge((216,432,316,547),2.1,.86)
    loft('spine | aft engineering service tray',[(555,20,1.99,2.23,2.46),(659,22,1.9,2.18,2.43),(782,14,1.64,1.84,2.01)])
    for label,side in [('port',-1),('starboard',1)]:
        outline=[(416,158),(438,168),(465,207),(465,345),(456,428),(421,453),
                 (401,433),(402,374),(405,262),(407,184)]
        slab('armor.crane.'+label+' | long main arm',outline,1.92,.67,side,bevel=.08)
        # Open foredeck has no concealed connecting polygon across the U.
        beam('structure.crane.'+label+' | fore articulated boom',point(CX+side*167,181,1.37),point(CX+side*99,91,1.48),.48,.60,silver)
        beam('structure.crane.'+label+' | brass actuator',point(CX+side*156,165,1.72),point(CX+side*100,102,1.81),.24,.18,brass)
        for j,(off,py) in enumerate(((167,182),(148,412)),1):
            zbody(f'crane.{label}.{j} | heavy pivot',CX+side*off,py,[(.39,.62),(.47,1.87),(.39,2.07)],graphite,40)
            zbody(f'crane.{label}.{j} | brass bearing cover',CX+side*off,py,[(.33,2.07),(.33,2.12)],brass,40)
        bounds=(min(CX+side*66,CX+side*118),64,max(CX+side*66,CX+side*118),149)
        box('cargo.tool.'+label+' | fore manipulator head',bounds,.55,2.10,bevel=.12)
        navigation(CX+side*87,105,2.11,label+'.tool')
        for j,(y0,y1) in enumerate(((491,559),(577,647)),1):
            xx=CX+side*182
            cargo_box(label,j,(xx-35,y0,xx+35,y1),.43,2.13)
            beam(f'structure.{label}.{j} | equipment rack mount',point(xx,(y0+y1)/2,1.21),point(CX+side*88,(y0+y1)/2,1.21),.47,.52,graphite)
        for py in (228,299,367):
            beam('structure.crane.'+label+' | open triangular bracing',point(CX+side*111,py+43,.99),point(CX+side*145,py,.99),.10,.13,graphite)
        beam('structure.crane.'+label+' | long hydraulic cylinder',point(CX+side*95,401,1.28),point(CX+side*128,281,1.32),.13,.18,silver)
        engine(label,CX+side*72,689,826,1.34,1.54,(1303 if side<0 else 1447,546),42)
    box('keel | ventral engineering spine',(245,472,289,750),-.06,.22,bevel=.07)

def make_medium_freighter():
    loft('spine | cargo transport keel',[(94,19,.4,1.54,1.82),(171,40,.21,1.79,2.04),
         (239,27,.1,1.65,1.91),(589,27,.1,1.64,1.89),(711,35,.15,1.58,1.83),(783,23,.34,1.39,1.65)])
    bridge((211,84,325,217),1.45,1.04)
    radial('sensor | forward probe',269,[(55,.045),(87,.06)],2.16,mat=silver,segments=24)
    for label,x,side in [('port',126,-1),('starboard',411,1)]:
        for i,(y0,y1) in enumerate(((229,352),(382,503),(534,655)),1):
            cargo_box(label,i,(x-87,y0,x+87,y1),.35,2.30)
            a,b=sorted((CX+side*29,x-side*86))
            truss(f'{label}.{i}',a,b,y0+36,y1-35,1.26,1.05)
            navigation(x+side*74,(y0+y1)/2,2.43,f'{label}.{i}')
    for py in (282,427,580):
        box('spine | dorsal lock module',(240,py-37,296,py+37),1.79,2.13,bevel=.07)
        box('keel | ventral service module',(240,py-37,296,py+37),-.05,.22,bevel=.05)
    loft('hull | aft drive yoke',[(644,35,.29,1.87,2.08),(689,86,.25,1.73,2.03),
         (731,122,.31,1.65,1.94),(778,91,.39,1.56,1.83)])
    for i,(px,u) in enumerate(((179,1245),(223,1303),(311,1418),(356,1478)),1):
        engine(str(i),px,705,810 if i in (2,3) else 806,.72,1.15,(u,548),20)

def make_authority_freighter():
    loft('spine | continuous authority carrier keel',[(77,27,.7,1.84,2.16),(116,95,.22,2.2,2.57),
         (204,115,.14,2.13,2.4),(301,42,.10,1.93,2.2),(557,43,.10,1.93,2.2),
         (636,108,.2,1.91,2.21),(697,167,.34,1.78,2.1),(748,37,.51,1.59,1.85)])
    bridge((219,77,332,150),2.14,.97)
    for label,side in [('port',-1),('starboard',1)]:
        for bank,(y0,y1) in enumerate(((204,389),(399,602)),1):
            x0,x1=(29,230) if side<0 else (313,498)
            # Navy perimeter cage surrounds six raised, individually editable tiles.
            box(f'structure.{label}.{bank} | cargo bank frame',(x0,y0,x1,y1),.29,1.98,bevel=.10)
            for row in range(3):
                for col in range(2):
                    a=x0+13+col*(x1-x0-24)/2;b=a+(x1-x0-24)/2-6
                    c=y0+15+row*(y1-y0-25)/3;d=c+(y1-y0-25)/3-5
                    name=f'cargo.{label}.{bank}.{row}.{col}'
                    box(name+' | top sealed cargo tile',(a,c,b,d),1.89,2.18,bevel=.045)
                    box(name+' | bottom cargo inspection tile',(a,c,b,d),.12,.40,bevel=.038)
                    for xx in (a+2,b-2):
                        box(name+' | brass locking dog',(xx-1.6,c+1,xx+1.6,c+6),2.17,2.22,bevel=.008,mat=brass)
            for py in (y0+10,y1-10):
                beam(f'structure.{label}.{bank} | perimeter rail',point(x0+6,py,2.02),point(x1-6,py,2.02),.12,.13,silver)
            navigation((x0+x1)/2,y0+4,2.12,f'{label}.{bank}')
        xx=CX+side*120
        box('structure.'+label+' | antenna root',(xx-9,124,xx+9,185),1.1,2.4,bevel=.08)
        antenna(xx,141,2.40,.56,label)
    for i,px in enumerate((101,203,337,439),1):
        u=(1145,1290,1451,1610)[i-1]
        engine(str(i),px,611,791,1.19,1.47,(u,556),33)
    for py in (267,447,626):
        box('spine | raised inspection spine',(250,py-51,298,py+51),2.04,2.33,bevel=.06)
        box('keel | ventral inspection spine',(250,py-51,298,py+51),-.02,.26,bevel=.055)
