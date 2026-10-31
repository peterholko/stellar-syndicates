"""Publish the recorded texture audit as a local, self-contained report UI."""
from pathlib import Path
import json
import html
from PIL import Image,ImageDraw,ImageFont

ROOT=Path(__file__).resolve().parent
AUDIT=ROOT/'texture-audit-v1'
SHIPS=['clean-slate','second-wind','small-fortune']
NAMES=['Clean Slate','Second Wind','Small Fortune']
OWNERS=['Ines Varga','Mags Halloran','Rafi Mensah']
DETAILS=[
 dict(findings=[
    'The cockpit side walls used a 12 × 10 pixel sample; the raised slab also put a second window belt below the top-view windows.',
    'The docking rims reached 28.9:1 local UV stretch. Shoulder edges and the central spine abruptly changed to unrelated swatches.',
    'Side-view docking artwork was vertically offset by about 34 pixels. Its original door crop was also off-center. Long wall polygons interpolated across a registration breakpoint, shifting features between their endpoints.'],
   fixes=[
    'Rebuilt the canopy as a sloped closed solid with one continuous native window projection.',
    'Unwrapped rings using physical circumference and radial cap coordinates. Folded the adjacent top-view pixels over thin panel edges.',
    'Registered side artwork to the actual collar center, resized the collars, and split wall geometry at the projection breakpoints.'],
   remaining='The long side/underside join and bow bevels still combine independently drawn views. Some panel lines and painted shading change across those joins; this is visible in close inspection.'),
 dict(findings=[
    'The workshop rim used an 8-pixel-wide orange crop: 98.65% of that component’s area fell below 8 texels per model unit. The crane arms used the same undersized sample.',
    'The wheelhouse had the same 12 × 10 pixel glass-wall problem. An intermediate concentric dome also placed its roof ridge through the windshield; this needed a separate feature-aligned cabin rebuild. The hidden starboard engine core reached 192.7:1 UV stretch.',
    'Three underside hatch faces silently replaced collapsed UVs with generic silver. Global bottom-view scaling was biased by the asymmetric bridge and crane.',
    'The side docking artwork was about 18 pixels too high, and nonlinear registration across unsplit wall faces displaced the circle along the hull. Its source crop center was slightly off too.'],
   fixes=[
    'Replaced tiny paint samples with native-resolution material strips and gave the crane/rim consistent physical texture scale.',
    'Traced independent wheelhouse roof and sill boundaries around the painted window frames, seated its console and antenna mounts, and continued the sill artwork onto its skirt. The exported windshield now occupies one plane. Engine body, core, ring and cap mappings are separate.',
    'Mapped the underside hatch walls explicitly, registered the belly to the rigid body centerline, and made collapsed UVs fail the build.',
    'Aligned the port-side projection and collar size; fitted distinct service-panel trims to starboard instead of projecting port-only docking details there.'],
   remaining='The concept’s mismatched engine housings and asymmetric underside are still interpreted geometry. The port engine retains a top/side projection boundary, and some belly/side plating transitions remain visible.'),
 dict(findings=[
    'The cockpit had a flat dark strip sampled from 12 × 10 pixels. Bow/front projection used a compressed, offset crop that put oversized details on the wrong faces.',
    'Engine coupling bands reached 70.2:1 local stretch. Cap UVs were normalized by the widest engine radius rather than the radius of the end face.',
    'The loading-lock walls and the edges of the cream spine used generic red paint. One belly-door wall silently fell back to silver.',
    'The side docking artwork was about 50 pixels too high. The long side projection also crossed a registration breakpoint inside an unsplit face, and the original source crop missed the center of the door.'],
   fixes=[
    'Rebuilt the sloped canopy and centered the lower-bow projection on the actual front body.',
    'Used radial cap mappings with the actual cap radius and physical scale for bands/nozzle walls; recentered the port exhaust crop and retained the native red/cream engine panels.',
    'Added fitted service-panel coverage to the loading lock, continued the cream stripe onto its raised edges, and mapped the belly-door wall explicitly.',
    'Aligned the port-side docking reference, resized and seated the collar against the hull, subdivided registration crossings and corrected the bottom-view centerline. The original housing started outside the body, leaving a 0.67-unit gap.'],
   remaining='The engine housings still combine native top and side illustrations, so narrow paint seams remain around those projection transitions. The starboard service trim is reconstructed coverage because there is no independent starboard drawing.')
]

records=[]
for slug,name,owner,details in zip(SHIPS,NAMES,OWNERS,DETAILS):
    before=json.loads((AUDIT/'before'/slug/'texture-audit.json').read_text())
    after=json.loads((AUDIT/'after'/slug/'texture-audit.json').read_text())
    info=json.loads((ROOT.parent/(slug+'-textured-v1')/'model-info.json').read_text())
    records.append(dict(slug=slug,name=name,owner=owner,before=before['aggregate'],after=after['aggregate'],
        fallbackBefore=len(before['silentUVFallbacks']),fallbackAfter=len(after['silentUVFallbacks']),
        dockingRegistrationBefore=before['portDockingRegistration'],dockingRegistrationAfter=after['portDockingRegistration'],
        seamCandidatesBefore=len(before['chartBoundaryColorSteps']),seamCandidatesAfter=len(after['chartBoundaryColorSteps']),
        modelSHA256=after['modelSHA256'],triangles=info['triangles'],**details))
(AUDIT/'summary.json').write_text(json.dumps({'revision':2,'ships':records},indent=2)+'\n')

def bullets(items):return '<ul>'+''.join('<li>'+html.escape(item)+'</li>' for item in items)+'</ul>'
sections=[]
for r in records:
    slug=r['slug']
    followup=('<p class="evidence"><a href="../command-module-repair/report.html">Second Wind command-module follow-up: close-ups and window alignment checks</a></p>' if slug=='second-wind' else '')
    sections.append(f'''<section class="ship-detail" id="{slug}"><div class="section-heading"><div><small>{r['owner']}</small><h2>{r['name']}</h2></div><a href="../../{slug}-textured-v1/viewer.html">Inspect repaired 3D model ↗</a></div>
<div class="two"><div><h3>Verified defects</h3>{bullets(r['findings'])}</div><div><h3>Repair applied</h3>{bullets(r['fixes'])}</div></div>
<p class="remaining"><strong>Remaining art limitation:</strong> {r['remaining']}</p>
{followup}
<p class="evidence"><a href="before/{slug}/nine-view-audit.png">Original nine-view sheet</a> · <a href="after/{slug}/nine-view-audit.png">Repaired nine-view sheet</a> · <a href="before/{slug}/texture-audit.json">Original measurements</a> · <a href="after/{slug}/texture-audit.json">Repaired measurements</a> · <a href="before/{slug}/{slug}.glb">Original GLB</a></p></section>''')
rows=[]
for r in records:
    b,a=r['before'],r['after']
    rows.append(f"<tr><th>{r['name']}</th><td>{b['areaWithAnisotropyOver4Pct']:.2f}% → {a['areaWithAnisotropyOver4Pct']:.2f}%</td><td>{b['areaBelow8TexelsPerUnitPct']:.2f}% → {a['areaBelow8TexelsPerUnitPct']:.2f}%</td><td>{b['maximumAnisotropy']:.1f}:1 → {a['maximumAnisotropy']:.2f}:1</td><td>{r['fallbackBefore']} → {r['fallbackAfter']}</td></tr>")
    rows.append(f"<tr><td colspan='5' class='note'>Port collar registration error: {r['dockingRegistrationBefore']['pixelError']:.1f} → {r['dockingRegistrationAfter']['pixelError']:.1f} source pixels, measured by interpolating exported UVs at the physical docking station.</td></tr>")

template='''<!doctype html><html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Texture audit · Independent traders</title><style>
*{box-sizing:border-box}body{margin:0;background:#0d131a;color:#e9e8e1;font:16px/1.65 system-ui,sans-serif}main{max-width:1400px;margin:auto;padding:50px 34px 90px}h1,h2,h3,p{margin-top:0}h1{font-size:44px;line-height:1.15;letter-spacing:-.025em;font-weight:550;max-width:900px;margin:16px 0 22px}h2{font-size:29px;line-height:1.2;margin:6px 0 18px;font-weight:550}h3{font-size:18px;margin-bottom:10px}small{color:#a7bbcb;font-size:12px;letter-spacing:.13em;text-transform:uppercase}header>p{max-width:940px;color:#b9c7d2;font-size:18px}.pill{display:inline-block;padding:5px 12px;border:1px solid #315e5d;background:#133130;color:#a7e4da;border-radius:20px;font-size:12px}.links{display:flex;gap:25px;flex-wrap:wrap;margin:24px 0 35px}a{color:#9adbd3;text-decoration:none}a:hover{text-decoration:underline}section{margin:34px 0;padding:30px;border:1px solid #2a3947;border-radius:14px;background:#111c26}.toolbar{display:flex;gap:15px;align-items:center;flex-wrap:wrap;margin-bottom:20px}label{font-size:13px;color:#b8c7d4}select{font:inherit;padding:8px 35px 8px 12px;color:#e7eee9;background:#1b2b38;border:1px solid #3b5262;border-radius:6px;margin-left:9px}.comparison,.two{display:grid;grid-template-columns:1fr 1fr;gap:24px}figure{margin:0;min-width:0}figure img{width:100%;display:block;border-radius:8px;background:#0a1015}figcaption{font-size:13px;color:#b9c7d2;margin:10px 0}table{border-collapse:collapse;width:100%;font-size:14px}th,td{text-align:left;padding:13px;border-bottom:1px solid #31404d}thead th{font-size:12px;color:#adc3d3;font-weight:500}tbody th{font-weight:500;white-space:nowrap}.tablewrap{overflow-x:auto}.note{font-size:13px;color:#a2b6c6;margin:18px 0 0}.section-heading{display:flex;justify-content:space-between;gap:20px;align-items:center}.section-heading a{font-size:13px}ul{padding-left:19px;margin:0}li{padding-left:4px;margin:0 0 11px;color:#b9c8d5}.remaining{background:#29281d;border-left:3px solid #c4aa68;padding:15px 18px;color:#ddd1ae;margin:20px 0}.evidence{font-size:13px;margin:0}.foot{color:#a8b8c6;font-size:14px}.two p{color:#b9c8d5}.qa{display:flex;gap:10px;flex-wrap:wrap;margin-bottom:20px}.qa span{font-size:12px;padding:6px 10px;background:#19322f;color:#b8e5d8;border-radius:5px}code{font-size:12px;overflow-wrap:anywhere}details{margin-top:20px;border-top:1px solid #324351;padding-top:15px}summary{cursor:pointer;color:#cbd6df}pre{white-space:pre-wrap;font-size:12px;color:#aebfce;line-height:1.6}@media(max-width:850px){main{padding:28px 16px}.comparison,.two{grid-template-columns:1fr}section{padding:20px}h1{font-size:32px}.section-heading{align-items:start;flex-direction:column}}</style>
<main><header><small>Stellar Syndicates / Independent traders / Audit revision 2</small><h1>UV mapping caused the visible texture defects.</h1><p>All three GLBs contain their base-color, metallic/roughness and emissive images. The untextured-looking areas came from tiny texture samples, misregistered projections, stretched UV charts and four silent material substitutions. The first file-validity checks did not detect those visual problems.</p><span class="pill">Major defects repaired · close-up art seams documented below</span><div class="links"><a href="../index.html">Open updated fleet gallery ↗</a><a href="../trader-ships-glb.zip">Download repaired GLBs ↓</a><a href="summary.json">Audit summary JSON</a></div></header>
<section><div class="toolbar"><label>Ship<select id="ship"><option value="clean-slate">Clean Slate · Ines</option><option value="second-wind">Second Wind · Mags</option><option value="small-fortune" selected>Small Fortune · Rafi</option></select></label><label>View<select id="view"><option>port-forward</option><option selected>starboard-forward</option><option>port-aft</option><option>starboard-aft</option><option>port</option><option>starboard</option><option>front</option><option>back</option><option>underside</option><option>uv-heatmap</option></select></label></div><div class="comparison"><figure><img id="before" alt="Original model rendered with its actual embedded texture"><figcaption>Before · saved original GLB</figcaption></figure><figure><img id="after" alt="Repaired model rendered with its actual embedded texture"><figcaption>After · repaired GLB, same camera and unlit inspection</figcaption></figure></div><p class="note">These are renders of the exported 3D files. The audit images are unlit so lighting cannot hide texture defects. UV heatmap: teal = within thresholds; yellow = stretch above 4:1; red = above 10:1; purple = below 8 texels per unit. Heatmap colors describe sampling quality, not semantic alignment.</p></section>
<section><h2>Measured changes</h2><div class="tablewrap"><table><thead><tr><th>Ship</th><th>Surface area above 4:1 stretch</th><th>Area below 8 texels/unit</th><th>Worst local stretch</th><th>Silent fallbacks</th></tr></thead><tbody>__ROWS__</tbody></table></div><p class="note">Computed from actual exported triangle positions and UVs using singular values of the UV-to-surface transform. Surface-area percentages include hidden component faces; the worst ratio may occur on a small fitting. These measurements establish sampling quality. They do not prove that every painted feature matches the geometry.</p></section>
__SECTIONS__
<section><h2>Validation and limits</h2><div class="qa"><span>3 embedded image maps per model</span><span>No collapsed UV charts</span><span>Original references preserved</span><span>Closed source components retained</span></div><div class="two"><div><h3>What was checked</h3><p>Nine unlit views per ship, lit overview renders, both sides, front and rear caps, underside coverage, UV density and distortion, shared-edge color changes, source hashes, feature counts, socket positions, and the actual cargo-cradle and engine-boom openings. Khronos glTF validation, the installed Three.js r180 loader and live browser rendering are checked independently.</p><p>The builder now rejects collapsed UVs instead of substituting silver silently. The viewer also exposes separate Port and Starboard views and applies anisotropic filtering at oblique angles.</p></div><div><h3>What remains</h3><p>These are static concept reconstructions. Their reference sheets contain independently painted views, including perspective cues and baked shadows. A shared-edge color audit still finds boundaries; some are intentional panel borders and others are projection transitions. All candidates remain in the per-ship JSON reports rather than being hidden behind a “perfect textures” claim.</p><p>A seamless close-up production asset would need a unified UV layout and a repaint/bake from coherent 3D surface detail. The current maps contain no dedicated normal or ambient-occlusion texture. The source-reference resolution also limits extreme close-up sharpness.</p></div></div><details><summary>Source assets, generated trims and reproducibility</summary><p class="foot">Original GLBs, model specifications, source builders and nine-view evidence are preserved under <code>texture-audit-v1/before/</code>. Repaired measurements are tied to the GLB SHA-256 hashes in <code>summary.json</code>. Supplemental paint and service trims were generated with the built-in image_gen tool, then packed at native resolution; the service strip was reduced to fit low side walls. The approved ship references were not repainted.</p><div class="links"><a href="source-collar-registration.png">Source-pixel registration grid</a><a href="../material-trims-v2.png">Saved generated material sheet</a><a href="../material-trims-v2.prompt.json">Exact generation prompt and tool</a><a href="../texture_mapping.py">UV repair source</a><a href="../audit_textures.py">Measurement source</a></div><pre>python3 client/art-src/ship-models/trader-fleet-v1/build_models.py
python3 client/art-src/ship-models/trader-fleet-v1/audit_textures.py --phase before
python3 client/art-src/ship-models/trader-fleet-v1/audit_textures.py --phase after
python3 client/art-src/ship-models/trader-fleet-v1/inspect_models.py
python3 client/art-src/ship-models/trader-fleet-v1/write_texture_report.py</pre></details></section><p class="foot">27 October 2026 · Repaired assets remain in their existing trader model folders. Original versions are retained in the audit archive.</p></main>
<script>const ship=document.querySelector('#ship'),view=document.querySelector('#view');function refresh(){for(const phase of ['before','after'])document.getElementById(phase).src=`${phase}/${ship.value}/${view.value}.png`;}ship.addEventListener('change',refresh);view.addEventListener('change',refresh);refresh();</script></html>'''
(AUDIT/'report.html').write_text(template.replace('__ROWS__',''.join(rows)).replace('__SECTIONS__',''.join(sections)))

# Directly comparable scientific-style inspection plate, built from GLB renders.
sheet=Image.new('RGB',(1800,1370),(13,19,26));draw=ImageDraw.Draw(sheet)
font=ImageFont.truetype('/System/Library/Fonts/Supplemental/Arial.ttf',22)
title=ImageFont.truetype('/System/Library/Fonts/Supplemental/Arial Bold.ttf',28)
draw.text((30,22),'TRADER TEXTURE AUDIT  /  ORIGINAL → REPAIRED',font=title,fill=(225,232,231))
for row,(slug,name) in enumerate(zip(SHIPS,NAMES)):
    y=90+row*420
    for col,phase in enumerate(['before','after']):
        im=Image.open(AUDIT/phase/slug/'starboard-forward.png');im.thumbnail((850,370))
        x=col*900+(900-im.width)//2
        sheet.paste(im,(x,y))
        draw.text((col*900+35,y+365),f'{name} · {phase}',font=font,fill=(171,195,208))
sheet.save(AUDIT/'before-after.png')
print('Wrote report.html, summary.json and before-after.png')
