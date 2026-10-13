import fs from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';
import {createRequire} from 'node:module';
const require=createRequire(new URL('../../../package.json',import.meta.url));
const sharp=require('sharp');
const record=JSON.parse(await fs.readFile(process.argv[2],'utf8'));
await fs.mkdir(path.dirname(record.output),{recursive:true});
try {await fs.access(record.output);throw new Error('Output already exists: '+record.output);} catch(e){if(e.code!=='ENOENT')throw e;}
const sha=b=>crypto.createHash('sha256').update(b).digest('hex');
const bounds=(data,w,h,threshold)=>{let x0=w,y0=h,x1=-1,y1=-1;for(let y=0;y<h;y++)for(let x=0;x<w;x++)if(data[(y*w+x)*4+3]>=threshold){x0=Math.min(x0,x);x1=Math.max(x1,x);y0=Math.min(y0,y);y1=Math.max(y1,y);}return [x0,y0,x1,y1];};
let bytes;
if(record.processing==='crop-hull'){
 const {data,info}=await sharp(record.source).ensureAlpha().raw().toBuffer({resolveWithObject:true});
 const [x0,y0,x1,y1]=bounds(data,info.width,info.height,240);
 const side=Math.round((y1-y0+1)/.89),left=Math.round((x0+x1+1-side)/2),top=Math.round((y0+y1+1-side)/2);
 if(left<0||top<0||left+side>info.width||top+side>info.height)throw new Error('Crop cannot preserve source pixels');
 bytes=await sharp(record.source).extract({left,top,width:side,height:side}).png().toBuffer();
 record.assembly={method:'Crop transparent canvas margins only; no pixel resampling or artificial upscale',left,top,side};
}else if(record.processing==='painting'){
 const meta=await sharp(record.source).metadata();
 if(meta.width===1671&&meta.height===941){bytes=await sharp(record.source).extend({right:1,extendWith:'copy'}).png().toBuffer();record.assembly={method:'Repeat the last edge column to meet exact 1672 x 941 canvas; no original generated pixel resampling'};}
 else if(meta.width===1672&&meta.height===941)bytes=await fs.readFile(record.source);
 else throw new Error('Unexpected painting size: '+meta.width+' x '+meta.height);
}else if(record.processing==='planet'){
 const {data,info}=await sharp(record.source).ensureAlpha().raw().toBuffer({resolveWithObject:true});
 const [x0,y0,x1,y1]=bounds(data,info.width,info.height,240);
 const scale=(1254*.91)/Math.max(x1-x0+1,y1-y0+1);
 if(scale>1)throw new Error('Planet normalization must not upscale');
 for(let i=3;i<data.length;i+=4)if(data[i]<=1)data[i]=0;
 const width=Math.round(info.width*scale),height=Math.round(info.height*scale);
 const resized=await sharp(data,{raw:{width:info.width,height:info.height,channels:4}}).resize(width,height).png().toBuffer();
 const left=Math.round(627-(x0+x1+1)/2*scale),top=Math.round(627-(y0+y1+1)/2*scale);
 bytes=await sharp({create:{width:1254,height:1254,channels:4,background:{r:0,g:0,b:0,alpha:0}}}).composite([{input:resized,left,top}]).png().toBuffer();
 record.assembly={method:'Uniformly downsample generated globe to 91% solid diameter; center on 1254 square transparent canvas; clear alpha-one noise only',scale,left,top};
}else if(record.processing==='orbit'){
 const original=await sharp(record.original).removeAlpha().raw().toBuffer({resolveWithObject:true});
 if(original.info.width!==1672||original.info.height!==941)throw new Error('Unexpected source framing');
 const w=1672,h=240;
 const sky=await sharp(record.source).resize(1672,1181,{fit:'fill'}).extract({left:0,top:0,width:w,height:h}).removeAlpha().raw().toBuffer();
 const delta=new Float64Array(w*3),low=new Float64Array(w*3);
 for(let x=0;x<w;x++)for(let c=0;c<3;c++)delta[x*3+c]=original.data[x*3+c]-sky[((h-1)*w+x)*3+c];
 for(let x=0;x<w;x++)for(let c=0;c<3;c++){let sum=0,weight=0;for(let dx=-24;dx<=24;dx++){let xx=Math.max(0,Math.min(w-1,x+dx)),a=Math.exp(-dx*dx/(2*8*8));sum+=delta[xx*3+c]*a;weight+=a;}low[x*3+c]=sum/weight;}
 const smooth=t=>{t=Math.max(0,Math.min(1,t));return t*t*t*(t*(t*6-15)+10);};
 for(let y=160;y<h;y++)for(let x=0;x<w;x++)for(let c=0;c<3;c++){let i=(y*w+x)*3+c,d=low[x*3+c]*smooth((y-160)/79)+(delta[x*3+c]-low[x*3+c])*smooth((y-236)/3);sky[i]=Math.round(Math.max(0,Math.min(255,sky[i]+d)));}
 const combined=Buffer.concat([sky,original.data]);
 bytes=await sharp(combined,{raw:{width:w,height:1181,channels:3}}).png().toBuffer();
 const preserved=await sharp(bytes).extract({left:0,top:240,width:1672,height:941}).raw().toBuffer();
 if(!preserved.equals(original.data))throw new Error('Original pixels changed');
 record.assembly={method:'Use generated top 240 rows, match seam within new sky only, place original unresampled at y=240',originalRowsUnchanged:true,changedOriginalPixels:0,sourcePixelSHA256:sha(original.data),outputOriginalRegionSHA256:sha(preserved)};
}else bytes=await fs.readFile(record.source);
await fs.writeFile(record.output,bytes,{flag:'wx'});
const meta=await sharp(bytes).metadata();
record.verification={width:meta.width,height:meta.height,channels:meta.channels,hasAlpha:meta.hasAlpha,fileSHA256:sha(bytes)};
if(meta.hasAlpha){const raw=await sharp(bytes).ensureAlpha().raw().toBuffer();let zero=0,partial=0;for(let i=3;i<raw.length;i+=4){if(!raw[i])zero++;else if(raw[i]<255)partial++;}record.verification.transparentPixels=zero;record.verification.partiallyTransparentPixels=partial;record.verification.solidBounds=bounds(raw,meta.width,meta.height,240);}
record.mode='builtin-imagegen';record.created='2026-10-09';
const promptPath=record.output.replace(/\.png$/,'.prompt.json');await fs.writeFile(promptPath,JSON.stringify(record,null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify({output:record.output,...record.verification,assembly:record.assembly},null,2));
