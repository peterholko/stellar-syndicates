"""Reproducible setup for the nine reference-derived ships. No Blender."""
from pathlib import Path
import json
import shutil

ROOT = Path(__file__).resolve().parent
MODELS = ROOT.parent
TEMPLATE = MODELS/'corvette-textured-v1'

# Native-pixel registration of the five drawings. Z is authored in model units.
SPECS = {
 'destroyer': dict(title='Destroyer', center=[270,445], top=[109,66,431,828], bottom=[460,29,794,828], side=[808,615,1738,795], front=[990,65,1575,300], back=[983,350,1579,578], sideBase=782, sideScale=38, frontBase=283, frontScale=53, backBase=563, backScale=53, engines=3, turrets=2),
 'battleship': dict(title='Battleship', center=[260,436], top=[15,52,505,825], bottom=[512,28,1009,825], side=[945,655,1747,826], front=[1075,40,1638,289], back=[1073,343,1640,605], sideBase=805, sideScale=38, frontBase=272, frontScale=45, backBase=597, backScale=46, engines=6, turrets=7),
 'dreadnought': dict(title='Dreadnought', center=[254,440], top=[16,64,490,817], bottom=[507,86,980,817], side=[988,652,1752,818], front=[1026,128,1719,326], back=[1020,377,1722,602], sideBase=808, sideScale=38, frontBase=316, frontScale=47, backBase=590, backScale=47, engines=5, turrets=17),
 'scout': dict(title='Scout', center=[307,418], top=[165,12,449,830], bottom=[578,12,860,830], side=[897,648,1734,814], front=[994,125,1652,299], back=[994,396,1656,568], sideBase=800, sideScale=45, frontBase=294, frontScale=60, backBase=562, backScale=42, engines=2, turrets=0),
 'colony-ship': dict(title='Colony Ship', center=[266,445], top=[20,46,514,844], bottom=[515,46,1005,844], side=[995,640,1757,819], front=[1007,68,1739,311], back=[1008,352,1742,606], sideBase=797, sideScale=33, frontBase=290, frontScale=45, backBase=583, backScale=45, engines=2, turrets=0),
 'troop-transport': dict(title='Troop Transport', center=[271,435], top=[27,68,516,804], bottom=[522,68,1011,804], side=[1013,636,1748,814], front=[1026,90,1739,313], back=[1025,353,1740,592], sideBase=796, sideScale=40, frontBase=302, frontScale=50, backBase=581, backScale=50, engines=2, turrets=0),
 'construction-ship': dict(title='Construction Ship', center=[267,444], top=[41,58,491,836], bottom=[518,57,977,836], side=[1003,647,1760,817], front=[1116,69,1631,314], back=[1100,353,1636,603], sideBase=801, sideScale=38, frontBase=306, frontScale=43, backBase=588, backScale=49, engines=2, turrets=0),
 'medium-freighter': dict(title='Medium Freighter', center=[268,437], top=[31,50,499,823], bottom=[517,50,979,823], side=[992,652,1746,820], front=[1026,76,1696,311], back=[1027,353,1695,581], sideBase=800, sideScale=40, frontBase=300, frontScale=54, backBase=573, backScale=48, engines=4, turrets=0),
 'authority-freighter': dict(title='Authority Freighter', center=[274,435], top=[16,66,505,801], bottom=[512,66,1001,801], side=[1002,649,1752,821], front=[1022,101,1729,319], back=[1022,360,1729,602], sideBase=802, sideScale=39, frontBase=306, frontScale=51, backBase=588, backScale=48, engines=4, turrets=0),
}

PROJECTION = '''# Registration is kept beside the deterministic model source.
def top_uv(v):
    return ((CX+v[0]*S)/2048,(CY-v[1]*S)/2048)

def bottom_uv(v):
    tx0,ty0,tx1,ty1=SPEC['top'];bx0,by0,bx1,by1=SPEC['bottom']
    # Bottom is viewed from below, so its transverse coordinate is mirrored.
    px=CX+v[0]*S;py=CY-v[1]*S
    return ((bx1-(px-tx0)/(tx1-tx0)*(bx1-bx0))/2048,
            (by0+(py-ty0)/(ty1-ty0)*(by1-by0))/2048)

def side_uv(v):
    x0,y0,x1,y1=SPEC['side'];a,b,c,d=SPEC['top']
    py=CY-v[1]*S
    return ((x0+(py-b)/(d-b)*(x1-x0))/2048,(SPEC['sideBase']-v[2]*SPEC['sideScale'])/2048)

def front_uv(v):
    x0,y0,x1,y1=SPEC['front'];a,b,c,d=SPEC['top']
    return (((x0+x1)/2-v[0]*S*(x1-x0)/(c-a))/2048,(SPEC['frontBase']-v[2]*SPEC['frontScale'])/2048)

def back_uv(v):
    # Rear-facing internal armor is occluded by engines in the drawing. Sample
    # its visible upper plating strip, never an exhaust circle. Actual recessed
    # exhaust faces use their own local rear-view UVs in engine().
    x0,y0,x1,y1=SPEC['backPlating'];a,b,c,d=SPEC['top']
    return (((x0+x1)/2+v[0]*S*(x1-x0)/(c-a))/2048,(y1-v[2]*(y1-y0)/4.8)/2048)

def hull_mapping(normal,center=None):
    if normal[2]<-.30:return ventral,bottom_uv
    if normal[2]>.30:return atlas,top_uv
    if abs(normal[0])>=abs(normal[1]):return atlas,side_uv
    return atlas,front_uv if normal[1]>0 else back_uv

def panel_uv(bounds,source):
    x0,y0,x1,y1=bounds;u0,v0,u1,v1=source
    return lambda v:((u0+(CX+v[0]*S-x0)/(x1-x0)*(u1-u0))/2048,(v0+(CY-v[1]*S-y0)/(y1-y0)*(v1-v0))/2048)

'''

def main():
    plating={'destroyer':[1053,432,1506,451],'battleship':[1170,440,1540,464],
             'dreadnought':[1100,433,1610,466],'scout':[1190,421,1443,445],
             'colony-ship':[1100,428,1600,451],'troop-transport':[1140,418,1590,443],
             'construction-ship':[1160,444,1580,468],'medium-freighter':[1200,429,1530,464],
             'authority-freighter':[1120,414,1615,452]}
    rear_panels={'destroyer':[1013,206,1093,268],'battleship':[1148,461,1201,526],
                 'dreadnought':[1150,433,1217,474],'scout':[1009,485,1039,530],
                 'colony-ship':[1642,453,1677,508],'troop-transport':[1543,441,1624,534],
                 'construction-ship':[1561,497,1620,559],'medium-freighter':[1060,444,1175,532],
                 'authority-freighter':[1090,425,1190,484]}
    for ship,spec in SPECS.items():
        spec['id']=ship
        spec['backPlating']=plating[ship]
        spec['rearPanel']=rear_panels[ship]
        spec['regions']={key:spec[key] for key in ('top','bottom','side','front','back')}
        out=MODELS/(ship+'-textured-v1');out.mkdir(exist_ok=True)
        (out/'model-spec.json').write_text(json.dumps(spec,indent=2)+'\n')
        core=(TEMPLATE/'mesh_core.py').read_text().replace('corvetteium','titanium')
        core=core.replace('S = 50.0\nCX, CY = 275.0, 435.0',"SPEC = json.loads((ROOT/'model-spec.json').read_text())\nS = 40.0\nCX, CY = SPEC['center']")
        a=core.index('# Plan drawings');b=core.index('def unit(a):',a)
        core=core[:a]+PROJECTION+core[b:]
        core=core.replace('    faces=[list(f) for f in faces]', '''    faces=[list(f) for f in faces]
    lo=vertices.min(axis=0);hi=vertices.max(axis=0)
    u0,v0,u1,v1=SPEC['rearPanel']
    def rear_component_uv(v):
        return ((u0+(v[0]-lo[0])/max(1e-8,hi[0]-lo[0])*(u1-u0))/2048,
                (v1-(v[2]-lo[2])/max(1e-8,hi[2]-lo[2])*(v1-v0))/2048)''')
        core=core.replace('        group=groups.setdefault(chosen,', '''        if chosen==atlas and projection is back_uv:projection=rear_component_uv
        group=groups.setdefault(chosen,''')
        core=core.replace(r'^(turret\.(?:port|starboard)\.[12])',r'^(turret\.[a-z]+\.\d+)')
        core=core.replace('four turret assemblies retained','independent turret assemblies retained')
        core=core.replace('corvette',ship).replace('Corvette',spec['title']).replace('CORVETTE',spec['title'].upper())
        (out/'mesh_core.py').write_text(core)
        texture=(TEMPLATE/'prepare_textures.py').read_text().replace('corvette',ship)
        a=texture.index('regions={');b=texture.index('for name,',a)
        texture=texture[:a]+"import json\nregions=json.loads((ROOT/'model-spec.json').read_text())['regions']\n"+texture[b:]
        # Expand within each cropped drawing to cover UV seams without altering art.
        texture=texture.replace('range(28)','range(36)')
        (out/'prepare_textures.py').write_text(texture)
        for name in ['geometry.py','build_preview.mjs','viewer.js','viewer-shell.html','validate.mjs','requirements.txt','package.json','THREE-LICENSE.txt']:
            src=TEMPLATE/name
            if not src.exists():continue
            text=src.read_text().replace('corvette',ship).replace('Corvette',spec['title']).replace('CORVETTE',spec['title'].upper())
            if name=='viewer.js':
                # All nine use the same plan scale; fit the longer 20-unit hulls.
                text=text.replace('?17.5', '?23.5').replace('?9.0','?15.0').replace("?13.0:17.0","?18.0:23.0")
                text=text.replace('camera.position.set(11,16,26)','camera.position.set(15,21,34)')
                text=text.replace("fetch('/capture/'+name+'.png'","fetch('/capture/"+ship+"/'+name+'.png'")
            (out/name).write_text(text)
        for name in ['build_model.py','designs.py']:
            shutil.copyfile(ROOT/name,out/name)
    print('Prepared nine standalone deterministic model packages.')

if __name__=='__main__':main()
