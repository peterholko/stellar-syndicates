// Promo screenshots of the production system and planet renderers. This
// isolated demonstration has no game connection or command sender.
import {Application,Assets} from 'pixi.js';
import {SystemViewScene,buildVisualSystem} from '../../src/systemview';
import {starTypeFor} from '../../src/stars';
import {starArtwork,starArtUrl} from '../../src/starart';
import {PlanetPanel} from '../../src/shell/deck/planet';
import {planetArtUrl} from '../../src/planetart';
import {label} from '../../src/icons';
import '../../src/styles/tokens.css';
import '../../src/styles/deck.css';
import '../../src/styles/planet-panel.css';
const $=id=>document.getElementById(id);
import {structures,bodies,system} from './world-demo.js';
if(new URLSearchParams(location.search).get('view')==='planet'){
  $('system').hidden=true;$('planet').hidden=false;
  const body=bodies[1];
  const report={id:system.id,name:system.name,owner:'promo',bodies,structures,builds:[],stockpile:[],module_stock:[],population:.12,
    assignments:Object.entries(structures).map(([structure,tier])=>({body_id:1,structure,tier,workers:tier,specialists:{},outputs:[],title:label(structure),staffing:1,skill:1})),
    workforce:{units:24,posted:16},blockade:null,age:0};
  const model={system,report,body,mine:true,art:planetArtUrl('terrestrial',512),delay:0,
    catalog:Object.keys(structures).map(key=>({key,label:label(key),costs:[],build_secs:60})),
    descriptions:Object.fromEntries(Object.keys(structures).map(key=>[key,label(key)])),workforceStructures:new Set(Object.keys(structures)),
    populationHtml:'',surveyHtml:'',constructionHtml:'',builderHtml:'',construction:null};
  const panel=new PlanetPanel();$('planet').innerHTML=panel.renderScene(model);
  const art=$('planet').querySelector('.deck-planet__surface');if(art)art.sizes='1920px';
  await Promise.all([...$('planet').querySelectorAll('img')].map(img=>img.decode().catch(()=>{})));
  document.title='Stellar Syndicates · Aster Prime structures';
}else{
  const width=$('capture').clientWidth,height=$('capture').clientHeight;
  const app=new Application();await app.init({width,height,resolution:1,antialias:true,background:'#050a12',preserveDrawingBuffer:true});
  $('system').append(app.canvas);
  const view=new SystemViewScene();app.stage.addChild(view.root);
  const star=starTypeFor(system.id),art=starArtwork('system',star.slug),texture=await Assets.load(starArtUrl('system',star.slug,1254));
  view.setSystem(buildVisualSystem(system,bodies),texture,art);view.root.visible=true;view.layout(width,height);view.zoomAt(width/2,height/2,1.65);
  app.ticker.add(()=>view.update('promo','promo',performance.now(),0));
  document.title='Stellar Syndicates · Aster system';
}
document.documentElement.dataset.ready='true';
