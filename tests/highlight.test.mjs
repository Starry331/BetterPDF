import {test} from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import vm from 'node:vm';
const source=readFileSync(new URL('../src-tauri/resources/engine/worker.js',import.meta.url),'utf8');
const ctx={fail:m=>{throw Error(m)}};vm.createContext(ctx);
vm.runInContext(source.slice(source.indexOf('function joinHighlight('),source.indexOf('function edit(')),ctx);
const q=(x,y,w=5)=>[x,y,x+w,y,x,y+10,x+w,y+10];
const merge=quads=>JSON.parse(JSON.stringify(ctx.highlightBands(quads)));
test('highlight bands merge reversed, overlapping and adjacent glyphs without filling gaps',()=>{
 assert.deepEqual(merge([q(10,0),q(0,0),q(5,0),q(5,0)]),[q(0,0,15)]);
 assert.equal(merge([q(0,0),q(0,20),q(40,0)]).length,3);
});
test('highlight bands follow rotated baselines',()=>{
 const rotate=q=>q.map((v,i)=>i%2?q[i-1]:-q[i+1]);
 assert.deepEqual(merge([rotate(q(5,0)),rotate(q(0,0))]),[rotate(q(0,0,10))].map(a=>a.map(v=>v===0?0:v)));
});
test('old per-character stacked highlights consolidate while comments and other colors survive',()=>{
 const annotation=(quads,color=[1,.82,.2],contents='')=>({getType:()=> 'Highlight',getContents:()=>contents,getOpacity:()=>.45,getColor:()=>color,getQuadPoints:()=>quads,setQuadPoints:q=>{quads=q},setColor(){},setOpacity(){},update(){}});
 const old=annotation([q(0,0),q(5,0)]),duplicate=annotation([q(0,0),q(5,0)]),comment=annotation([q(0,0)],undefined,'review'),blue=annotation([q(0,0)],[0,0,1]);
 let annotations=[old,duplicate,comment,blue];
 ctx.addHighlight({getAnnotations:()=>annotations,deleteAnnotation:a=>annotations=annotations.filter(b=>a!==b)},[q(5,0),q(10,0)],[1,.82,.2]);
 assert.equal(annotations.length,3);assert(annotations.includes(comment));assert(annotations.includes(blue));assert.deepEqual(JSON.parse(JSON.stringify(old.getQuadPoints())),[q(0,0,15)]);
});
