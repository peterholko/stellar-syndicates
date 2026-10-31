import fs from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';
import {fileURLToPath} from 'node:url';
import {createRequire} from 'node:module';
import {slides,duration} from './storyboard.mjs';
const dir=path.dirname(fileURLToPath(import.meta.url));
const root=path.resolve(dir,'../../..');
const require=createRequire(new URL('../ship-models/pirate-fleet-v1/package.json',import.meta.url));
const esbuild=require('esbuild');
const images={
  heroLogo:'client/public/art/derived/login/stellar-syndicates-banner-v1-1440.webp',
  nebula:'client/public/art/nebulas/molecular_cloud.png',
  colony:'client/public/art/derived/story-events/v1/new-colony-1672.webp',
  shipyard:'client/public/art/derived/shipyard-interiors/v1/shipyard-ii-empty-bay-1672.webp',
  slipway:'client/public/art/derived/shipyard-interiors/v1/capital-slipway-empty-bay-1672.webp',
  relics:'client/public/art/derived/story-events/v1/relics-awaken-1672.webp',
  alliances:'client/public/art/derived/lore/rival_fleet_standoff-1280.webp',
  mystery:'client/public/art/derived/story-events/v1/sealed-world-found-1672.webp',
  system:'client/art-src/promo-slideshow-v1/assets/star-system-orbits-poster.png',
  planet:'client/art-src/promo-slideshow-v1/assets/planet-structures-screenshot.jpg',
  combat:'client/art-src/promo-slideshow-v1/assets/fleet-combat-poster.png',
};
const models={hub:'client/art-src/ship-models/wormhole-hub-textured-v1/wormhole-hub.glb'};
for(const id of ['interceptor','cruiser','titan','medium-freighter','small-freighter','tiny-freighter'])
  models[id]=`client/public/art/derived/ship-models/v1/${id}-lod2.glb`;
const records=[];
async function encode(p){
  const b=await fs.readFile(path.join(root,p));
  records.push({path:p,bytes:b.length,sha256:crypto.createHash('sha256').update(b).digest('hex')});
  return b.toString('base64');
}
const assets={images:{},models:{},video:{}};
for(const [key,p] of Object.entries(images)) assets.images[key]=`data:image/${p.endsWith('.webp')?'webp':/\.jpe?g$/.test(p)?'jpeg':'png'};base64,${await encode(p)}`;
for(const [key,p] of Object.entries(models)) assets.models[key]=await encode(p);
assets.video.system=await encode('client/art-src/promo-slideshow-v1/output/stellar-syndicates-star-system-orbits.mp4');
assets.video.combat=await encode('client/art-src/promo-slideshow-v1/output/stellar-syndicates-fleet-combat.mp4');
const evidence=await fs.readFile(path.join(root,'client/public/dev/battles/mixed.json'));
records.push({path:'client/public/dev/battles/mixed.json',bytes:evidence.length,sha256:crypto.createHash('sha256').update(evidence).digest('hex'),use:'Recorded combat engine evidence, rounds 45–69'});
for(const p of ['client/art-src/promo-slideshow-v1/world-demo.js','client/art-src/promo-slideshow-v1/orbits-capture.js','client/src/systemview.ts','client/src/systemlayout.ts']){
  const source=await fs.readFile(path.join(root,p));
  records.push({path:p,bytes:source.length,sha256:crypto.createHash('sha256').update(source).digest('hex'),use:'Orbital time-lapse source, 12 seconds at 40x orbital time'});
}
const result=await esbuild.build({entryPoints:[path.join(dir,'player.js')],bundle:true,minify:true,format:'iife',
  platform:'browser',target:['es2022'],write:false,legalComments:'inline',
  nodePaths:[path.join(root,'client/node_modules')]});
const template=await fs.readFile(path.join(dir,'template.html'),'utf8');
const html=template.replaceAll('__DURATION__',String(duration)).replaceAll('__SCENE_COUNT__',String(slides.length)).replaceAll('__DURATION_LABEL__',`${Math.floor(duration/60)}:${String(duration%60).padStart(2,'0')}`)
  .replace('__ASSETS__',JSON.stringify(assets).replaceAll('<','\\u003c'))
  .replace('__PLAYER__',()=>result.outputFiles[0].text.replaceAll('</script','<\\/script'));
await fs.writeFile(path.join(dir,'index.html'),html);
await fs.writeFile(path.join(dir,'asset-manifest.json'),JSON.stringify({title:'Stellar Syndicates',durationSeconds:duration,
  slideCount:slides.length,canvas:{width:1920,height:1080},assets:records,slides},null,2)+'\n');
await fs.copyFile(path.join(root,'client/node_modules/three/LICENSE'),path.join(dir,'THREE-LICENSE.txt'));
console.log(`Built ${slides.length} scenes / ${duration}s / ${(Buffer.byteLength(html)/1048576).toFixed(1)} MiB. Self-contained, no network requests.`);
