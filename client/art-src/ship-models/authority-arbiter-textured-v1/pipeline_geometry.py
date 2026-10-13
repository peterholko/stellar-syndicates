"""Reusable triangulation from the existing Authority Nexus reconstruction pipeline."""
import numpy as np

def unit(v):
    v = np.asarray(v, dtype=float)
    return v / max(np.linalg.norm(v), 1e-12)

def triangulate(vertices):
    if len(vertices) == 3:
        return [(0, 1, 2)]
    normal = np.sum(np.cross(vertices, np.roll(vertices, -1, axis=0)), axis=0)
    xy = np.delete(vertices, np.argmax(np.abs(normal)), axis=1)
    area = np.sum(xy[:, 0]*np.roll(xy[:, 1], -1)-np.roll(xy[:, 0], -1)*xy[:, 1])
    sign = 1 if area >= 0 else -1
    cross = lambda a, b: a[0]*b[1]-a[1]*b[0]
    remaining = list(range(len(vertices)))
    result = []
    while len(remaining) > 3:
        for k, b in enumerate(remaining):
            a, c = remaining[k-1], remaining[(k+1) % len(remaining)]
            if sign*cross(xy[b]-xy[a], xy[c]-xy[b]) < 1e-12:
                continue
            if any(all(sign*q > 1e-10 for q in (
                cross(xy[b]-xy[a], xy[j]-xy[a]),
                cross(xy[c]-xy[b], xy[j]-xy[b]),
                cross(xy[a]-xy[c], xy[j]-xy[c]))) for j in remaining if j not in (a, b, c)):
                continue
            result.append((a, b, c))
            remaining.pop(k)
            break
        else:
            raise ValueError('Non-simple or degenerate polygon')
    result.append(tuple(remaining))
    return result
