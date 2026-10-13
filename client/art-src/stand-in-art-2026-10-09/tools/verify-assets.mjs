import fs from 'node:fs/promises';
import path from 'node:path';
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import {fileURLToPath} from 'node:url';
import {createRequire} from 'node:module';
const require=createRequire(new URL('../../../package.json',import.meta.url));
const sharp=require('sharp');
const art=fileURLToPath(new URL('../../',import.meta.url));
const output=fileURLToPath(new URL('../',import.meta.url));
const sha=b=>crypto.createHash('sha256').update(b).digest('hex');
const images=[
 ['survey-probe-reference-v1/survey-probe.png',1193,1193,true],
 ['survey-probe-reference-v1/survey-probe-reference.png',1774,887,false],
 ['planets-2026-10-08/marginal-ocean-v1.png',1254,1254,true],
 ...[
  'planet-view/surfaces/marginal-ocean-surface-v1',
  'planet-view/habitable-v1/boreal-world-v2',
  'planet-view/habitable-v1/volcanic-highlands-v2',
  'planet-view/habitable-v1/archipelago-world-v2',
  'planet-view/surfaces/ocean-surface-v1-v2',
 ].flatMap(stem=>[[stem+'.png',1672,941,false],[stem+'-orbit.png',1672,1181,false]])
];
const entries=[],prompts=[];
for(const [file,width,height,alpha]of images){
 const filename=path.join(art,file),bytes=await fs.readFile(filename),meta=await sharp(bytes).metadata();
 assert.equal(meta.width,width,file);assert.equal(meta.height,height,file);assert.equal(meta.hasAlpha,alpha,file);
 const record=JSON.parse(await fs.readFile(filename.replace('.png','.prompt.json'),'utf8'));
 assert.equal(record.verification.fileSHA256,sha(bytes),file+' prompt hash');
 const entry={file,width,height,alpha,bytes:bytes.length,sha256:sha(bytes),prompt:file.replace('.png','.prompt.json')};
 if(alpha){
  const raw=await sharp(bytes).ensureAlpha().raw().toBuffer();let transparent=0;
  for(let i=3;i<raw.length;i+=4)if(raw[i]===0)transparent++;
  assert(transparent>100000,file+' lacks empty transparent padding');entry.zeroAlphaPixels=transparent;
 }
 if(file.endsWith('-orbit.png')){
  const base=file.replace('-orbit.png','.png');
  const source=await sharp(path.join(art,base)).removeAlpha().raw().toBuffer();
  const region=await sharp(bytes).extract({left:0,top:240,width:1672,height:941}).removeAlpha().raw().toBuffer();
  assert(source.equals(region),file+' changed lower 941 rows');
  entry.sourcePainting=base;entry.originalRowsUnchanged=true;entry.changedOriginalPixels=0;entry.preservedPixelSHA256=sha(region);
 }
 entries.push(entry);prompts.push({asset:file,...record});
}
const modelDir=path.join(art,'ship-models/survey-probe-textured-v1');
const info=JSON.parse(await fs.readFile(path.join(modelDir,'model-info.json'),'utf8'));
const gltf=JSON.parse(await fs.readFile(path.join(modelDir,'gltf-validation.json'),'utf8'));
const surface=JSON.parse(await fs.readFile(path.join(modelDir,'surface-verification.json'),'utf8'));
const three=JSON.parse(await fs.readFile(path.join(modelDir,'three-validation.json'),'utf8'));
assert.equal(info.triangles,2448);assert.equal(info.renderSections,2);
assert.equal(gltf.issues.numErrors,0);assert.equal(gltf.issues.numWarnings,0);
assert(surface.allPhysicalTrianglesTextured&&surface.allPhysicalTrianglesHaveAreaUVs);
assert.equal(surface.closedComponents,50);assert.equal(three.threeRevision,'180');
const glbBytes=await fs.readFile(path.join(modelDir,'survey-probe.glb'));
const placements=JSON.parse(await fs.readFile(path.join(output,'placement-verification.json'),'utf8'));
assert(placements.every(r=>r.cleanSites>=17));
const manifest={date:'2026-10-09',brief:'docs/stand-in-art-prompts.md',imageGenerationMode:'builtin-imagegen',
 primaryImageCount:entries.length,images:entries,
 model:{file:'ship-models/survey-probe-textured-v1/survey-probe.glb',bytes:glbBytes.length,sha256:sha(glbBytes),
  triangles:info.triangles,renderSections:info.renderSections,closedComponents:surface.closedComponents,
  allPhysicalTrianglesTextured:true,texturedDownwardTriangles:surface.texturedDownwardTriangles,
  gltfErrors:0,gltfWarnings:0,threeRevision:three.threeRevision,
  engineSocket:info.sockets[0],generator:'Python/NumPy reconstruction, Mining Fleet pipeline, no Blender'},
 placementChecks:placements,
 verification:'Decoded image dimensions/alpha, SHA-256, exact lower-region RGB comparisons, exported GLB geometry/UV checks, Khronos validator and Node Three.js loader. Ocean placement reviews exclude reflective water and floating ice.',
 integrationStatus:'Art-only handoff; catalog, runtime sprites, model scaling and placement table wiring remain a separate task.'};
await fs.writeFile(path.join(output,'manifest.json'),JSON.stringify(manifest,null,2)+'\n');
await fs.writeFile(path.join(output,'prompts.json'),JSON.stringify({mode:'builtin-imagegen',assets:prompts},null,2)+'\n');
console.log(JSON.stringify({primaryImages:entries.length,transparentSprites:entries.filter(e=>e.alpha).length,
 pixelExactOrbitExtensions:entries.filter(e=>e.originalRowsUnchanged).length,
 modelTriangles:info.triangles,modelSections:info.renderSections,gltfErrors:0,gltfWarnings:0,
 cleanLandSites:placements.map(p=>({world:p.id,count:p.cleanSites}))},null,2));
