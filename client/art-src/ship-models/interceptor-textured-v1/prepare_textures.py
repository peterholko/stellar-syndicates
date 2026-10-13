"""Create texture channels from the approved reference without repainting its art."""
from pathlib import Path
import shutil
import numpy as np
from PIL import Image

ROOT = Path(__file__).resolve().parent
SOURCE = ROOT.parents[1] / "interceptor-reference-v1/interceptor-reference.png"
OUT = ROOT / "textures"
OUT.mkdir(exist_ok=True)
if not SOURCE.exists():
    SOURCE = OUT / "reference-sheet.png"
if SOURCE.resolve() != (OUT / "reference-sheet.png").resolve():
    shutil.copyfile(SOURCE, OUT / "reference-sheet.png")
source = np.asarray(Image.open(SOURCE).convert("RGB"), dtype=np.float32)
albedo = np.full_like(source, (74, 78, 82))
# The panels retain their original pixel positions, making the projections editable.
# Padding only extends edge colors into unused background to prevent white UV seams.
for x0, y0, x1, y1 in [(15, 75, 686, 896), (700, 290, 1495, 440), (692, 695, 1518, 854)]:
    tile = source[y0:y1, x0:x1].copy()
    known = tile.min(axis=2) < 237
    for _ in range(16):
        padded = np.pad(tile * known[..., None], ((1, 1), (1, 1), (0, 0)))
        valid = np.pad(known.astype(np.float32), 1)
        total = np.zeros_like(tile)
        count = np.zeros_like(known, dtype=np.float32)
        h, w = known.shape
        for dy, dx in [(0, 1), (1, 0), (1, 2), (2, 1), (0, 0), (0, 2), (2, 0), (2, 2)]:
            total += padded[dy:dy+h, dx:dx+w]
            count += valid[dy:dy+h, dx:dx+w]
        fill = (~known) & (count > 0)
        tile[fill] = total[fill] / count[fill, None]
        known |= fill
    tile[~known] = (74, 78, 82)
    albedo[y0:y1, x0:x1] = tile
rgb = np.clip(albedo, 0, 255).astype(np.uint8)
Image.fromarray(rgb).save(OUT / "interceptor-basecolor.png")
r, g, b = (albedo[..., i] for i in range(3))
cyan = np.clip((g-r-42)/100, 0, 1) * np.clip((b-r-52)/100, 0, 1)
emission = albedo * cyan[..., None]
Image.fromarray(np.clip(emission, 0, 255).astype(np.uint8)).save(OUT / "interceptor-emissive.png")
brightness = albedo.mean(axis=2) / 255
roughness = np.where(brightness > .55, .57, .48)
metallic = np.where(brightness > .55, .38, .30)
glass = (b > r*1.09) & (brightness < .36) & (cyan < .1)
roughness[glass] = .25
metallic[glass] = .20
metallic[cyan > .3] = .08
orm = np.stack([np.ones_like(r), roughness, metallic], axis=2)
Image.fromarray(np.clip(orm*255, 0, 255).astype(np.uint8)).save(OUT / "interceptor-metallic-roughness.png")
print("Prepared exact-reference color atlas, emission, and metallic/roughness channels.")
