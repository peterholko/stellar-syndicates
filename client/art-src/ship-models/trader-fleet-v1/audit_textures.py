"""Audit actual texture sampling and UV distortion beyond glTF file validity."""
from pathlib import Path
import argparse
import copy
import json
import hashlib
import importlib.util
import sys
from collections import defaultdict
from types import SimpleNamespace

import numpy as np
from PIL import Image,ImageDraw,ImageFont

ROOT=Path(__file__).resolve().parent
sys.path.insert(0,str(ROOT.parent/'mining-fleet-v1'))
from render_core import Model,render

SHIPS=['clean-slate','second-wind','small-fortune']
VIEWS=[('port-forward',(-1.3,.75,-1.5)),('starboard-forward',(1.3,.75,-1.5)),
       ('port-aft',(-1.3,.60,1.5)),('starboard-aft',(1.3,.60,1.5)),
       ('port',(-1,0,0)),('starboard',(1,0,0)),('front',(0,0,-1)),
       ('back',(0,0,1)),('underside',(1,-1,-1.3))]


def distortions(p):
    v=p['p'][p['i']].astype(float)
    uv=p['uv'][p['i']].astype(float)*2048
    e1,e2=v[:,1]-v[:,0],v[:,2]-v[:,0]
    l1=np.linalg.norm(e1,axis=1)
    x2=(e1*e2).sum(1)/l1
    y2=np.sqrt(np.maximum((e2*e2).sum(1)-x2*x2,1e-20))
    physical=np.zeros((len(v),2,2))
    physical[:,0,0]=l1;physical[:,0,1]=x2;physical[:,1,1]=y2
    tex=np.stack([uv[:,1]-uv[:,0],uv[:,2]-uv[:,0]],axis=2)
    jac=tex@np.linalg.inv(physical)
    singular=np.linalg.svd(jac,compute_uv=False)
    area=np.linalg.norm(np.cross(e1,e2),axis=1)/2
    return dict(area=area,anisotropy=singular[:,0]/np.maximum(singular[:,1],1e-12),
        density=np.sqrt(singular.prod(axis=1)),minimumDensity=singular[:,1])


def collect(model):
    per_component=defaultdict(lambda:defaultdict(list))
    for p in model.primitives:
        d=distortions(p)
        texel=np.floor(p['uv'][p['i']].mean(1)*2048).astype(int).clip(0,2047)
        color=model.images[0][texel[:,1],texel[:,0]]*255
        d['atlasBackground']=np.all(np.abs(color-[52,58,65])<.6,axis=1)
        for c in p['components']:
            start=c['indexStart']//3;end=start+c['indexCount']//3
            for key,value in d.items():per_component[c['name']][key].extend(value[start:end].tolist())
    results=[]
    for name,raw in per_component.items():
        d={k:np.asarray(v) for k,v in raw.items()};area=d['area'];total=area.sum()
        pct=lambda condition:round(float(100*area[condition].sum()/total),3)
        results.append(dict(component=name,triangles=len(area),surfaceArea=round(float(total),4),
            areaWithAnisotropyOver4Pct=pct(d['anisotropy']>4),
            areaWithAnisotropyOver10Pct=pct(d['anisotropy']>10),
            areaBelow8TexelsPerUnitPct=pct(d['minimumDensity']<8),
            areaOnAtlasBackgroundPct=pct(d['atlasBackground']),
            minimumTexelsPerUnit=round(float(d['minimumDensity'].min()),4),
            medianTexelsPerUnit=round(float(np.median(d['density'])),3),
            maximumAnisotropy=round(float(d['anisotropy'].max()),3)))
    return sorted(results,key=lambda r:r['surfaceArea']*(r['areaWithAnisotropyOver4Pct']+r['areaBelow8TexelsPerUnitPct']),reverse=True)


def collect_seams(model):
    """Screen same-component shared edges for discontinuous base-color charts.

    Color steps also occur at intentional material borders. This inventory is
    evidence for visual review, not an automatic pass/fail or texture-loss test.
    """
    edges=defaultdict(list)
    for primitive in model.primitives:
        for c in primitive['components']:
            first=c['indexStart']//3;last=first+c['indexCount']//3
            for indices in primitive['i'][first:last]:
                for a,b in [(0,1),(1,2),(2,0)]:
                    p=primitive['p'][indices[[a,b]]]
                    uv=primitive['uv'][indices[[a,b]]]
                    keys=[tuple(np.round(q,5)) for q in p]
                    if keys[1]<keys[0]:keys.reverse();p=p[::-1];uv=uv[::-1]
                    edges[(c['name'],*keys)].append((p,uv))
    image=model.images[0]*255
    h,w=image.shape[:2]
    def sample(uv):
        q=(uv*np.array([w,h])-.5).clip([0,0],[w-1.001,h-1.001])
        p=np.floor(q).astype(int);t=q-p
        return (image[p[:,1],p[:,0]]*(1-t[:,0,None])*(1-t[:,1,None])+
            image[p[:,1],p[:,0]+1]*t[:,0,None]*(1-t[:,1,None])+
            image[p[:,1]+1,p[:,0]]*(1-t[:,0,None])*t[:,1,None]+
            image[p[:,1]+1,p[:,0]+1]*t[:,0,None]*t[:,1,None])
    findings=[]
    for key,pair in edges.items():
        if len(pair)!=2:continue
        (p,a),(_,b)=pair
        length=float(np.linalg.norm(p[1]-p[0]))
        if length<.3 or np.max(np.abs(a-b))<1e-5:continue
        t=np.linspace(.08,.92,9)[:,None]
        delta=float(np.abs(sample(a[0]*(1-t)+a[1]*t)-sample(b[0]*(1-t)+b[1]*t)).mean())
        if delta>45:findings.append(dict(component=key[0],length=round(length,4),
            meanRGBStep=round(delta,2),modelEdge=p.round(4).tolist()))
    return sorted(findings,key=lambda f:f['length']*f['meanRGBStep'],reverse=True)


def docking_registration(model,slug):
    """Check UV interpolation at the physical port collar center on hull faces."""
    body,station,cy,expected={
        'clean-slate':('hull.enclosed-freight-body',435,436,[1363,719]),
        'second-wind':('hull.workshop',443,437,[1225,728]),
        'small-fortune':('hull.spade-body',321,429,[1210,701])}[slug]
    point=np.array([.05,(station-cy)/38])
    hits=[]
    for p in model.primitives:
        for component in p['components']:
            if component['name']!=body:continue
            start=component['indexStart']//3;end=start+component['indexCount']//3
            for indices in p['i'][start:end]:
                tri=p['p'][indices].astype(float)
                normal=np.cross(tri[1]-tri[0],tri[2]-tri[0])
                if normal[0]>=-.55*np.linalg.norm(normal):continue
                yz=tri[:,[1,2]]
                matrix=np.column_stack([yz[1]-yz[0],yz[2]-yz[0]])
                if abs(np.linalg.det(matrix))<1e-10:continue
                u,v=np.linalg.solve(matrix,point-yz[0])
                if u>=-1e-6 and v>=-1e-6 and u+v<=1+1e-6:
                    bary=np.array([1-u-v,u,v])
                    tex=bary@p['uv'][indices]*2048
                    hits.append(dict(sampledPixel=tex.round(3).tolist(),expectedPixel=expected,
                        pixelError=round(float(np.linalg.norm(tex-expected)),3),
                        hullX=float(bary@tri[:,0])))
    if not hits:raise AssertionError(f'{slug}: no hull surface at port docking station')
    result=min(hits,key=lambda hit:hit['hullX'])
    housing=[]
    for p in model.primitives:
        for c in p['components']:
            if c['name']=='docking.port.housing':
                first=c['indexStart']//3;last=first+c['indexCount']//3
                housing.extend(p['p'][p['i'][first:last]].reshape(-1,3)[:,0].tolist())
    result['housingRootGapUnits']=round(max(0,result['hullX']-max(housing)),5)
    result['method']='Barycentric interpolation of exported hull UVs at port collar height and longitudinal station.'
    return result


def inspect_fallbacks(slug,phase):
    """Instrument the existing builder before its silent UV fallback runs."""
    if phase=='before':
        filename=ROOT/'texture-audit-v1'/'before'/'build_models.py'
        module_spec=importlib.util.spec_from_file_location('baseline_trader_build',filename)
        build_models=importlib.util.module_from_spec(module_spec)
        module_spec.loader.exec_module(build_models)
    else:
        import build_models
    from reconstruction import Reconstruction
    from pipeline_geometry import unit,triangulate
    original=Reconstruction.mesh
    report=[]
    def checked(self,name,vertices,faces,style='auto',mapping=None,smooth=False,assembly='hull'):
        v=np.asarray(vertices,float)
        fs=[list(f) for f in faces]
        volume=sum(np.dot(v[f[0]],np.cross(v[f[j]],v[f[j+1]])) for f in fs for j in range(1,len(f)-1))/6
        if volume<0:fs=[f[::-1] for f in fs]
        for fi,f in enumerate(fs):
            points=v[f]
            n=unit(np.sum(np.cross(points,np.roll(points,-1,axis=0)),axis=0))
            if mapping:uv=mapping(points,n,fi)
            elif style=='auto' and abs(n[2])>.55:uv=self.plan_uv(points,n[2]<0)
            else:uv=self.face_uv(points,n,'silver' if style=='auto' else style)
            ts=uv[np.asarray(triangulate(points))]
            a,b=ts[:,1]-ts[:,0],ts[:,2]-ts[:,0]
            if np.abs(a[:,0]*b[:,1]-a[:,1]*b[:,0]).min()<1e-12:
                report.append(dict(component=name,face=fi,fallback='silver' if style=='auto' else style,
                    worldCenter=points.mean(0).round(4).tolist()))
        return original(self,name,vertices,faces,style,mapping,smooth,assembly)
    Reconstruction.mesh=checked
    try:
        folder=ROOT.parent/(slug+'-textured-v1')
        spec_path=(ROOT/'texture-audit-v1'/'before'/slug/'model-spec.json') if phase=='before' else folder/'model-spec.json'
        spec=json.loads(spec_path.read_text())
        m=build_models.Trader(folder,spec)
        build_models.BUILDERS[slug](m)
    finally:Reconstruction.mesh=original
    return report


def unlit(model):
    other=copy.copy(model)
    other.primitives=[]
    for p in model.primitives:
        q=dict(p);q['m']=copy.deepcopy(p['m'])
        q['m'].setdefault('extensions',{})['KHR_materials_unlit']={}
        q['m']['emissiveFactor']=[0,0,0]
        other.primitives.append(q)
    return other


def heatmap(model):
    # A diagnostic render of UV distortion, not a texture edit.
    colors=np.array([[.14,.42,.46],[.90,.65,.14],[.89,.20,.18],[.49,.29,.83]],np.float32)
    palette=np.repeat(np.repeat(colors[None,:,:],16,0),16,1)
    other=SimpleNamespace(images=[palette],doc=dict(textures=[{'source':0}],samplers=[]),
        positions=model.positions,primitives=[])
    for p in model.primitives:
        d=distortions(p)
        category=np.where(d['anisotropy']>10,2,np.where(d['anisotropy']>4,1,0))
        category=np.where(d['minimumDensity']<8,3,category)
        vertex=p['p'][p['i']].reshape(-1,3)
        uv=np.stack([(category+.5)/4,np.full(len(category),.5)],axis=1).repeat(3,axis=0)
        other.primitives.append(dict(p=vertex,n=p['n'][p['i']].reshape(-1,3),uv=uv,
            i=np.arange(len(vertex)).reshape(-1,3),m={'pbrMetallicRoughness':{'baseColorTexture':{'index':0}},
                'extensions':{'KHR_materials_unlit':{}}},name=p['name'],components=[]))
    return other


def audit(slug,phase,draw=True):
    folder=ROOT.parent/(slug+'-textured-v1')
    path=ROOT/'texture-audit-v1'/'before'/slug/(slug+'.glb') if phase=='before' else folder/(slug+'.glb')
    model=Model(path)
    components=collect(model)
    report={'ship':slug,'phase':phase,'source':str(path),'missingMaterialTextureSlots':0,
        'modelSHA256':hashlib.sha256(path.read_bytes()).hexdigest(),
        'method':'Per-triangle UV-to-world singular values and surface-area weighting on exported GLB. Metrics include occluded component faces; visual checks determine visible severity.',
        'components':components}
    report['chartBoundaryColorSteps']=collect_seams(model)
    report['portDockingRegistration']=docking_registration(model,slug)
    if phase=='after':
        assert report['portDockingRegistration']['pixelError']<3,(slug,report['portDockingRegistration'])
        assert report['portDockingRegistration']['housingRootGapUnits']<.02,(slug,'floating docking housing')
    report['chartBoundaryNote']='Shared edges longer than 0.3 units with mean RGB step above 45/255; includes intentional borders. Review visually. This is not a missing-texture count.'
    for p in model.primitives:
        pbr=p['m'].get('pbrMetallicRoughness',{})
        report['missingMaterialTextureSlots']+=sum(k not in pbr for k in ['baseColorTexture','metallicRoughnessTexture'])
        report['missingMaterialTextureSlots']+=int('emissiveTexture' not in p['m'])
    report['silentUVFallbacks']=inspect_fallbacks(slug,phase)
    area=sum(c['surfaceArea'] for c in components)
    report['aggregate']={'triangles':sum(c['triangles'] for c in components),
        'minimumTexelsPerUnit':min(c['minimumTexelsPerUnit'] for c in components),
        'maximumAnisotropy':max(c['maximumAnisotropy'] for c in components)}
    for key in ['areaWithAnisotropyOver4Pct','areaWithAnisotropyOver10Pct','areaBelow8TexelsPerUnitPct','areaOnAtlasBackgroundPct']:
        report['aggregate'][key]=round(sum(c['surfaceArea']*c[key] for c in components)/area,3)
    if phase=='after':
        assert report['aggregate']['maximumAnisotropy']<4,(slug,'UV stretch regression')
        assert report['aggregate']['minimumTexelsPerUnit']>=8,(slug,'UV density regression')
        assert not report['silentUVFallbacks'],(slug,'silent material fallback')
        assert report['missingMaterialTextureSlots']==0,(slug,'missing texture slot')
    out=ROOT/'texture-audit-v1'/phase/slug
    out.mkdir(parents=True,exist_ok=True)
    (out/'texture-audit.json').write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps({'ship':slug,'phase':phase,'missingTextureSlots':report['missingMaterialTextureSlots'],
        'silentFallbackFaces':len(report.get('silentUVFallbacks',[])),
        'worstComponents':components[:8]}),flush=True)
    if not draw:return
    lit=unlit(model)
    for name,camera in VIEWS:
        print(f'Rendering {phase} {slug}: {name}',flush=True)
        render(lit,camera,width=1000,height=650).save(out/(name+'.png'))
    debug=heatmap(model)
    render(debug,(1.3,.75,-1.5),width=1000,height=650).save(out/'uv-heatmap.png')
    sheet=Image.new('RGB',(1800,1440),(12,16,23))
    draw=ImageDraw.Draw(sheet)
    font=ImageFont.truetype('/System/Library/Fonts/Supplemental/Arial.ttf',20)
    draw.text((22,18),f'{slug.upper()} / {phase.upper()} / UNLIT TEXTURE INSPECTION',font=font,fill=(232,232,230))
    for i,(name,_) in enumerate(VIEWS):
        x,y=(i%3)*600,60+(i//3)*450
        im=Image.open(out/(name+'.png'));im.thumbnail((590,390))
        sheet.paste(im,(x+(600-im.width)//2,y))
        draw.text((x+20,y+400),name.upper(),font=font,fill=(186,200,216))
    sheet.save(out/'nine-view-audit.png')


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--phase',choices=['before','after'],default='before')
    parser.add_argument('--metrics-only',action='store_true')
    parser.add_argument('ships',nargs='*')
    args=parser.parse_args()
    for slug in args.ships or SHIPS:audit(slug,args.phase,not args.metrics_only)
