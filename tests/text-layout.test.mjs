import {test} from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import vm from 'node:vm';
const worker=readFileSync(new URL('../src-tauri/resources/engine/worker.js',import.meta.url),'utf8');
const code=worker.slice(worker.indexOf('function scalars('),worker.indexOf('function removeText('));
function harness(){
 const streams=[],codes=[];
 const dict=()=>({put(){},get(){}});
 const ctx={fail:m=>{throw Error(m)},Font:function(){return {encodeCharacter(c){codes.push(c);return c===0x20000?7:c;},advanceGlyph(){return 1;}}},resources:dict,dictCopy:dict,Matrix:{invert:()=>[1,0,0,1,0,0]}};
 vm.createContext(ctx);vm.runInContext(code,ctx);
 const doc={countObjects:()=>1,addObject:dict,addFont:dict,addStream:s=>{streams.push(s);return s;}};
 return {ctx,streams,codes,render:(text,box=[0,0,100,100])=>ctx.textContent(doc,{getTransform(){}},text,box,12,[0,0,0],1)};
}
test('supplementary characters remain one glyph with UTF-16 ToUnicode mapping',()=>{const h=harness();const out=h.render('𠀀');assert(out.includes('<0007>'));assert(h.streams.some(s=>s.includes('<0007> <d840dc00>')));assert(h.codes.every(c=>c===0x20000));});
test('single glyph wider than box fails instead of escaping its bounds',()=>{assert.throws(()=>harness().render('W',[0,0,5,100]),/TEXT_OVERFLOW/);});
test('CRLF and CR normalize to line breaks, malformed surrogates fail',()=>{const h=harness();assert.equal((h.render('a\r\nb\rc').match(/ Tj ET/g)||[]).length,3);assert.throws(()=>h.render('\ud800'),/Invalid Unicode/);assert.throws(()=>h.render('\udc00'),/Invalid Unicode/);});
