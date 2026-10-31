"""Prepared transparency cleanup. Run with --apply only after user authorization.

The saved generation attempts are immutable inputs. This performs no drawing,
resizing, relighting, or image generation. It removes the green matte, unmattes
only antialiased edge pixels, and restores original RGB pixels in unchanged
opaque interiors for tiers 1–4 and 6. New docking clamps retain edited pixels.
Without --apply, it only prints the proposed cleanup measurements.
"""

from pathlib import Path
import argparse
import hashlib
import json

import numpy as np
from PIL import Image
from scipy import ndimage


ROOT = Path(__file__).resolve().parent
CLAMP_EDITS = {
    1: [(1320, 355, 1490, 475)],
    2: [(1320, 355, 1500, 455)],
    3: [(1320, 315, 1610, 445), (1400, 425, 1600, 555)],
    4: [(1320, 380, 1660, 675)],
    6: [(100, 510, 300, 690), (1370, 620, 1660, 830), (1250, 170, 1430, 260)],
}


def prepare(entry):
    tier = entry['tier']
    source = Path(entry['attempts'][-1]['saved_path'])
    rgb = np.asarray(Image.open(source).convert('RGB')).copy()
    assert rgb.shape == (941, 1672, 3), (source, rgb.shape)
    color = rgb.astype(np.float64)
    border = np.concatenate((color[0], color[-1], color[:, 0], color[:, -1]))
    background = np.median(border, axis=0)
    green_excess = color[:, :, 1] - np.maximum(color[:, :, 0], color[:, :, 2])
    background_excess = background[1] - max(background[0], background[2])
    assert background_excess > 150, (source, background)

    matte_alpha = np.clip(1.0 - green_excess / background_excess, 0.0, 1.0)
    alpha = matte_alpha.copy()
    # Dark green left in narrow structural openings is also background.
    # Saturation, rather than brightness, distinguishes it from dark rock.
    strongest_other = np.maximum(color[:, :, 0], color[:, :, 2])
    green_background = (green_excess > 20) & (
        (strongest_other + 1) / (color[:, :, 1] + 1) < 0.45
    )
    alpha[green_background] = 0.0
    alpha[alpha <= 0.12] = 0.0
    alpha[alpha >= 0.96] = 1.0
    # Dark colors are not keyed. Interior rock and solid structures are opaque.
    core = ndimage.binary_erosion(alpha > 0.2, iterations=2)
    alpha[core & (green_excess < 35)] = 1.0

    # Remove disconnected matte noise without filling real structural openings.
    labels, count = ndimage.label(alpha > 0, structure=np.ones((3, 3)))
    assert count > 0, source
    areas = np.bincount(labels.ravel())
    areas[0] = 0
    main_component = int(np.argmax(areas))
    alpha[labels != main_component] = 0
    # Partial coverage is reserved for the immediate antialiased boundary.
    interior = ndimage.distance_transform_edt(alpha > 0) > 1.5
    alpha[interior] = 1.0

    output = color.copy()
    spill = (alpha > 0) & (green_excess > 0)
    spill_alpha = np.maximum(matte_alpha[spill, None], 0.12)
    output[spill] = np.clip(
        (color[spill] - (1 - spill_alpha) * background) / spill_alpha, 0, 255
    )
    # Correct residual green fringes from the generator's uneven green matte.
    output[:, :, 1] = np.minimum(
        output[:, :, 1], np.maximum(output[:, :, 0], output[:, :, 2])
    )

    restored_count = 0
    if tier != 5:
        original = np.asarray(Image.open(entry['input']).convert('RGB'))
        assert original.shape == rgb.shape
        restore = ndimage.binary_erosion(alpha == 1, iterations=4)
        for x0, y0, x1, y1 in CLAMP_EDITS[tier]:
            restore[y0:y1, x0:x1] = False
        output[restore] = original[restore]
        restored_count = int(restore.sum())

    output = np.rint(output).astype(np.uint8)
    output[alpha == 0] = 0
    alpha8 = np.rint(alpha * 255).astype(np.uint8)
    rgba = np.dstack((output, alpha8))
    assert np.all(alpha8[0] == 0) and np.all(alpha8[-1] == 0)
    assert np.all(alpha8[:, 0] == 0) and np.all(alpha8[:, -1] == 0)
    shadow_points = [(650, 750), (850, 850), (1100, 700)]
    shadow_alpha = [int(alpha8[y, x]) for x, y in shadow_points]
    assert shadow_alpha == [255, 255, 255], (tier, shadow_alpha)
    report = {
        'tier': tier,
        'filename': f'tier-{tier}.png',
        'format': 'PNG',
        'size': [1672, 941],
        'mode': 'RGBA',
        'transparent_pixels': int(np.count_nonzero(alpha8 == 0)),
        'opaque_pixels': int(np.count_nonzero(alpha8 == 255)),
        'antialiased_edge_pixels': int(np.count_nonzero((alpha8 > 0) & (alpha8 < 255))),
        'restored_original_interior_pixels': restored_count,
        'shadow_sample_alpha': shadow_alpha,
        'all_canvas_edges_transparent': True,
        'source_attempt': str(source),
    }
    return rgba, report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--apply', action='store_true', help='Write six final PNGs after authorization.')
    args = parser.parse_args()
    manifest = json.loads((ROOT / 'prompts.json').read_text())
    reports = []
    for entry in manifest['outputs']:
        rgba, report = prepare(entry)
        if args.apply:
            path = ROOT / report['filename']
            Image.fromarray(rgba).save(path, format='PNG')
            report['sha256'] = hashlib.sha256(path.read_bytes()).hexdigest()
        reports.append(report)
    if args.apply:
        manifest['finalBackground'] = 'transparent, with fully opaque solid interiors'
        manifest['finalValidationStatus'] = 'alpha corrected; final visual review required'
        manifest['postprocessing'] = {
            'script': 'cleanup_alpha.py',
            'authorization': 'User explicitly authorized scripted image cleanup.',
            'operations': [
                'Chroma-key generated green background without keying dark rock.',
                'Set solid interiors to alpha 255 and empty space to alpha 0.',
                'Unmatte antialiased silhouette pixels; remove isolated matte noise.',
                'Restore unchanged original RGB interiors except edited docking-clamp regions.',
            ],
        }
        (ROOT / 'prompts.json').write_text(json.dumps(manifest, indent=2) + '\n')
        (ROOT / 'validation.json').write_text(json.dumps({
            'status': 'alpha_corrected_visual_review_pending',
            'assets': reports,
        }, indent=2) + '\n')
    print(json.dumps({'applied': args.apply, 'assets': reports}, indent=2))


if __name__ == '__main__':
    main()
