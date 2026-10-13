// Read-only art QA: reuse the game's existing classifier and placement algorithm
// on new versioned paintings. Never call buildPlanetSites or change game tables.
import fs from 'node:fs/promises';
import path from 'node:path';
import {pathToFileURL,fileURLToPath} from 'node:url';
import {createRequire} from 'node:module';
const root=fileURLToPath(new URL('../../../../',import.meta.url)).replace(/\/$/,'');
const require=createRequire(root+'/client/package.json');
const script=root+'/client/scripts/build-planet-sites.mjs';
let code=await fs.readFile(script,'utf8');
code=code.replace('import sharp from "sharp";',`import sharp from ${JSON.stringify(pathToFileURL(require.resolve('sharp')).href)};`)
 .replace('async function classify(', 'export async function classify(')
 .replace('function placeColony(', 'export function placeColony(')
 .replace('async function review(', 'export async function review(');
const moduleFile=root+'/tmp/imagegen/site-analysis-core.mjs';
await fs.writeFile(moduleFile,code);
const {classify,placeColony,review}=await import(pathToFileURL(moduleFile));
const cases=[
 ['boreal-world','habitable-v1/boreal-world-v2.png',{vegetated:true}],
 ['volcanic-highlands','habitable-v1/volcanic-highlands-v2.png',{vegetated:true}],
 ['archipelago-world','habitable-v1/archipelago-world-v2.png',{vegetated:true}],
 ['ocean','surfaces/ocean-surface-v1-v2.png',{}],
 ['marginal-ocean','surfaces/marginal-ocean-surface-v1.png',{}]
];
const out=root+'/client/art-src/stand-in-art-2026-10-09';
await fs.mkdir(out+'/placement-review',{recursive:true});
const results=[];
for(const [id,file,opts]of cases){
 const source=root+'/client/art-src/planet-view/'+file;
 const cls=await classify(source,opts);
 // Reflective water is nearly white at the left edge and can fool the existing
 // hue classifier. Exclude surrounding sea/ice from these island-only reviews;
 // retain the original per-pixel classifications and thresholds within the band.
 const islandOnly=id==='ocean'||id==='marginal-ocean';
 if(islandOnly)for(let y=0;y<470;y++)for(let x=0;x<836;x++)
   if(x<836*.17||x>836*.94||y<470*.47||y>470*.75)cls[y*836+x]=1;
 const {sites,spacing}=placeColony(cls);
 const clean=sites.filter(s=>s.tier<=1);
 results.push({id,source:file,cleanSites:clean.length,generatedAddresses:sites.length,spacing,
  cleanCoordinates:clean.map(s=>[+(s.x/836*100).toFixed(1),+(s.y/470*100).toFixed(1)]),
  assessment:'Existing terrain classifier, original thresholds and footprint scale; art QA only, not game integration.',
  regionConstraint:islandOnly?'Main island band only: x=17–94%, y=47–75%; excludes specular sea and floating-ice false positives.':null});
 await review(source,cls,sites,out+'/placement-review/'+id+'.jpg');
 console.log(id+': '+clean.length+'/24 clean sites');
}
await fs.writeFile(out+'/placement-verification.json',JSON.stringify(results,null,2)+'\n');
