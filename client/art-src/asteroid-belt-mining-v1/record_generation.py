"""Copy generated masters without changing pixels; inspect alpha and provenance."""
from pathlib import Path
from hashlib import sha256
import json
import shutil
import sys
import numpy as np
from PIL import Image

ROOT = Path(__file__).resolve().parents[3]
HERE = Path(__file__).resolve().parent
asset_id, source_path = sys.argv[1:3]
plan = json.loads((HERE/'generation-plan.json').read_text())
asset = next(a for a in plan['assets'] if a['id'] == asset_id)
source, target = Path(source_path), ROOT/asset['file']
image = Image.open(source)
rgba = np.asarray(image.convert('RGBA'))
alpha = rgba[:, :, 3]
ys, xs = np.where(alpha > 10)
report = dict(id=asset_id, file=asset['file'], generatedSource=str(source),
              mode=image.mode, dimensions=list(image.size),
              sha256=sha256(source.read_bytes()).hexdigest(),
              alphaRange=[int(alpha.min()), int(alpha.max())],
              transparentPixelFraction=float(np.mean(alpha == 0)),
              opaquePixelFraction=float(np.mean(alpha == 255)),
              subjectBounds=[int(xs.min()), int(ys.min()), int(xs.max()+1), int(ys.max()+1)],
              borderFullyTransparent=bool(not alpha[0].any() and not alpha[-1].any() and
                                          not alpha[:, 0].any() and not alpha[:, -1].any()))
report['borderMaxAlpha'] = int(max(alpha[0].max(), alpha[-1].max(), alpha[:, 0].max(), alpha[:, -1].max()))
if asset['transparent']:
    assert image.mode == 'RGBA' and alpha.min() == 0 and alpha.max() == 255, 'Missing genuine alpha'
    assert report['transparentPixelFraction'] > .05, 'Insufficient transparent background'
    assert report['borderMaxAlpha'] <= 8, 'Visible content touches the canvas border'
target.parent.mkdir(parents=True, exist_ok=True)
shutil.copy2(source, target)
manifest_path = HERE/'generation-results.json'
manifest = json.loads(manifest_path.read_text()) if manifest_path.exists() else {'mode': 'built-in image_gen', 'assets': []}
manifest['assets'] = [r for r in manifest['assets'] if r['id'] != asset_id]+[report]
manifest_path.write_text(json.dumps(manifest, indent=2)+'\n')
print(json.dumps(report))
