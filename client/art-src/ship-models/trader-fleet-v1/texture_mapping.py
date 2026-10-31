"""Component-aware UV charts for the trader texture repair.

Plan drawings belong to planar deck/belly faces. Hull walls use purpose-made
material strips; circular parts use axial/radial charts with physical scale.
An invalid chart is a build error, never an invisible material substitution.
"""
import math
import numpy as np
from pipeline_geometry import unit, triangulate


class TextureMapping:
    def split_registration_planes(self,vertices,faces):
        """Make piecewise view registration linear within each exported face.

        GPUs interpolate UVs between vertices. Merely sampling a nonlinear
        registration at the ends of a long wall moves its middle features.
        Splits retain the original triangular surface and closed topology.
        """
        source=np.asarray(vertices,float)
        polygons=[source[np.asarray(face)][np.asarray(tri)] for face in faces for tri in triangulate(source[np.asarray(face)])]
        planes=[(self.spec['center'][1]-y)/38 for y in self.spec['sideRegistration'][0][1:-1]]
        for plane in planes:
            updated=[]
            for poly in polygons:
                distances=poly[:,1]-plane
                if distances.min()>=-1e-9 or distances.max()<=1e-9:
                    updated.append(poly);continue
                for sign in (-1,1):
                    clipped=[]
                    for a,b in zip(poly,np.roll(poly,-1,axis=0)):
                        da,db=sign*(a[1]-plane),sign*(b[1]-plane)
                        if da>=-1e-9:clipped.append(a)
                        if (da>1e-9 and db<-1e-9) or (da<-1e-9 and db>1e-9):
                            clipped.append(a+(b-a)*(plane-a[1])/(b[1]-a[1]))
                    if len(clipped)>=3:updated.append(np.array(clipped))
            polygons=updated
        points=[];indices={};result=[]
        for poly in polygons:
            face=[]
            for point in poly:
                key=tuple(np.round(point,8))
                if key not in indices:indices[key]=len(points);points.append(point)
                if not face or face[-1]!=indices[key]:face.append(indices[key])
            if face[-1]==face[0]:face.pop()
            if len(face)>=3:result.append(face)
        return points,result

    def plan_uv(self, points, bottom=False):
        pixels=np.column_stack([self.spec['center'][0]+points[:,0]*38,
                                self.spec['center'][1]-points[:,1]*38])
        if bottom:
            # Register the rigid body centerline and engine/cradle stations.
            # Crane/wheelhouse silhouette extents are not body-scale anchors.
            if self.spec['id']=='second-wind':
                pixels[:,0]=758-(pixels[:,0]-278)*.972
                pixels[:,1]=56+(pixels[:,1]-56)*.999
            elif self.spec['id']=='small-fortune':
                pixels[:,0]=754-(pixels[:,0]-262)*.986
                pixels[:,1]+=1
            else:
                pixels[:,0]=733-(pixels[:,0]-246)*.984
        return pixels/2048

    def face_uv(self, points, normal, patch):
        """Stable world axes and one density for each whole-component chart."""
        normal=unit(normal)
        dominant=int(np.argmax(np.abs(normal)))
        if dominant==2:
            horizontal=np.array([1.,0,0]);vertical=np.array([0.,-1,0])
        elif dominant==0:
            horizontal=np.array([0.,-1,0]);vertical=np.array([0.,0,-1])
        else:
            horizontal=np.array([1.,0,0]);vertical=np.array([0.,0,-1])
        basis=np.column_stack([horizontal,vertical])
        bounds=getattr(self,'_chart_vertices',points)@basis
        center=(bounds.min(0)+bounds.max(0))/2
        extent=np.maximum(np.ptp(bounds,axis=0),1e-6)
        rect=np.asarray(self.spec['patches'][patch],float).reshape(2,2)
        scale=min(80 if patch=='glass' else 38, float(np.min((rect[1]-rect[0]-8)/extent)))
        if patch=='service':
            scale=np.minimum([38,48],(rect[1]-rect[0]-8)/extent)
        return ((rect[0]+rect[1])/2+(points@basis-center)*scale)/2048

    def hull_wall_uv(self,p,n,name):
        if name.startswith('ventral.'):
            return self.face_uv(p,n,'dark')
        if abs(n[0])>.55:
            py=self.spec['center'][1]-p[:,1]*38
            stations,pixels=self.spec['sideRegistration']
            u=np.interp(py,stations,pixels)
            # Register the visible docking disk to its real geometry center.
            # The old "baseline" was close to the drawing's keel, not z=0.
            docking_pixel={'clean-slate':719,'second-wind':728,'small-fortune':701}[self.spec['id']]
            v=docking_pixel-(p[:,2]-.05)*self.spec['sideHeightScale']
            if n[0]>0 and self.spec['id']!='clean-slate':
                return self.face_uv(p,n,'service')
            return np.column_stack([u,v])/2048
        if n[1]>.55:
            if self.spec['id']=='small-fortune':
                return np.column_stack([1374-p[:,0]*38,231-p[:,2]*30])/2048
            if self.spec['id']=='clean-slate':
                return np.column_stack([1360-p[:,0]*50,230-p[:,2]*38])/2048
            return self.face_uv(p,n,'paint')
        if n[1]<-.55 and 'tail' not in name:
            region=self.spec['regions']['back']
            # Match the rigid center rather than changing scale for attachments.
            scale={'clean-slate':50,'second-wind':46,'small-fortune':42}[self.spec['id']]
            return np.column_stack([(region[0]+region[2])/2+p[:,0]*scale,
                self.spec['backBaseline']-p[:,2]*self.spec['elevationScale']])/2048
        return self.face_uv(p,n,'paint')

    def mesh(self,name,vertices,faces,style='auto',mapping=None,smooth=False,assembly='hull'):
        if mapping is None and style=='auto' and name.startswith('hull.') and '.pylon.' not in name and 'loading-' not in name:
            vertices,faces=self.split_registration_planes(vertices,faces)
        self._chart_vertices=np.asarray(vertices,float)
        if mapping is None:
            def mapping(p,n,fi):
                if style!='auto':return self.face_uv(p,n,style)
                if 'loading-' in name:
                    return self.face_uv(p,n,'service')
                if name=='bridge.glazing':
                    if n[2]>.08:return self.plan_uv(p)
                    if n[2]<-.985:return self.face_uv(p,n,'dark')
                    if self.spec['id']=='second-wind':
                        # The short cabin skirt continues its adjacent sill,
                        # rather than acquiring an unrelated olive paint band.
                        direction=unit(np.array([n[0],-n[1]]))
                        return self.plan_uv(p)-(p[:,2].max()-p[:,2])[:,None]*38*direction/2048
                    return self.face_uv(p,n,'paint')
                if '.pylon.' in name or name.endswith('engine-mount'):
                    return self.face_uv(p,n,'dark')
                if (abs(n[2])<=.55 and np.ptp(self._chart_vertices[:,2])<=.42 and
                    name.startswith(('cargo.','armor.','machinery.','cradle.','engine.','bridge.'))):
                    # Fold the adjacent deck pixels down a thin panel edge.
                    # This keeps its paint stripe continuous at the top seam,
                    # rather than switching a cream spine to generic red paint.
                    direction=unit(np.array([n[0],-n[1]]))
                    inset=(p[:,2].max()-p[:,2])*38
                    return self.plan_uv(p)-inset[:,None]*direction/2048
                if name=='engine.starboard.housing' and self.spec['id']=='second-wind' and abs(n[0])>.55:
                    py=self.spec['center'][1]-p[:,1]*38
                    return np.column_stack([1607+(py-646)/165*141,734-p[:,2]*40])/2048
                if abs(n[2])>.55:
                    return self.plan_uv(p,n[2]<0)
                if name.startswith(('hull.','ventral.')):
                    return self.hull_wall_uv(p,n,name)
                if name.startswith(('cargo.','armor.','cradle.','bridge.','engine.')):
                    return self.face_uv(p,n,'paint')
                return self.face_uv(p,n,'silver')
        chosen=mapping
        def checked(p,n,fi):
            uv=np.asarray(chosen(p,n,fi),float)
            triangles=uv[np.asarray(triangulate(p))]
            a,b=triangles[:,1]-triangles[:,0],triangles[:,2]-triangles[:,0]
            area=np.abs(a[:,0]*b[:,1]-a[:,1]*b[:,0])
            if not np.isfinite(uv).all() or area.min()<1e-12:
                raise ValueError(f'{name} face {fi}: invalid UV chart (no silent material fallback)')
            if uv.min()<0 or uv.max()>1:raise ValueError(f'{name}: UV outside atlas')
            return uv
        super().mesh(name,vertices,faces,style,checked,smooth,assembly)

    def lathe(self,name,center,axis,rows,segments=24,style='auto',wall='silver',cap=None,ring=False):
        center,axis=np.asarray(center,float),unit(axis)
        helper=[0,1,0] if abs(axis[2])>.9 else [0,0,1]
        right=unit(np.cross(axis,helper));up=unit(np.cross(axis,right))
        vertices=[center+axis*h+r*(right*math.cos(i*math.tau/segments)+up*math.sin(i*math.tau/segments))
                  for h,r in rows for i in range(segments)]
        count=len(rows) if ring else len(rows)-1
        faces=[(k*segments+i,k*segments+(i+1)%segments,((k+1)%len(rows))*segments+(i+1)%segments,
                ((k+1)%len(rows))*segments+i) for k in range(count) for i in range(segments)]
        if not ring:faces += [tuple(reversed(range(segments))),tuple(range(len(vertices)-segments,len(vertices)))]
        rmax=max(r for _,r in rows);hmin=min(h for h,_ in rows);hmax=max(h for h,_ in rows)
        rect=np.asarray(self.spec['patches'][wall],float).reshape(2,2)
        midpoint=rect.mean(0)
        scale=min(38,float((rect[1,0]-rect[0,0]-8)/(math.tau*rmax)),
                  float((rect[1,1]-rect[0,1]-8)/max(hmax-hmin,1e-8)))
        radial_scale=min(38,float(np.min(rect[1]-rect[0]-8)/(2*rmax)))
        def mapping(p,n,fi):
            local=p-center
            axial=local@axis
            radial=np.column_stack([local@right,-local@up])
            is_cap=not ring and fi>=count*segments
            if is_cap and fi==count*segments+1 and cap:
                cx,cy,pixel_radius=cap
                # The source disk fills the actual end face, not the widest
                # point of a tapered nozzle elsewhere along this component.
                return (np.array([cx,cy])+radial/rows[-1][1]*pixel_radius)/2048
            if is_cap and fi==count*segments and name.endswith('.housing') and self.spec['id']=='small-fortune':
                return (np.array([1137,205])+radial/rows[0][1]*42)/2048
            # The engine body's native projections carry its characteristic
            # red/cream or salvaged panels. Keep them away from thin rings,
            # planar caps and the hidden starboard core, which have own charts.
            if name.endswith('.housing') and name.startswith('engine.') and not is_cap:
                if abs(n[2])>.65:return self.plan_uv(p,n[2]<0)
                if abs(n@axis)<.8:
                    x0,y0,x1,y1=self.spec['engineSide']
                    u=x0+(axial-hmin)/max(hmax-hmin,1e-8)*(x1-x0)
                    v=(y0+y1)/2-(p[:,2]-center[2])*38
                    return np.column_stack([u,v])/2048
            if is_cap or abs(n@axis)>.80:
                return (midpoint+radial*radial_scale)/2048
            angles=np.unwrap(np.arctan2(local@up,local@right))
            if angles.min() < -1e-7:angles+=math.tau
            horizontal=(angles-math.pi)*rmax
            vertical=(hmin+hmax)/2-axial
            return (midpoint+np.column_stack([horizontal,vertical])*scale)/2048
        self.mesh(name,vertices,faces,style,mapping=mapping,smooth=True)
