import {test} from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import vm from 'node:vm';
import ts from 'typescript';
const app=readFileSync(new URL('../src/App.svelte',import.meta.url),'utf8');
const source=ts.transpileModule(app.slice(app.indexOf(' async function finishQuit('),app.indexOf(' async function saveCopy(')),{compilerOptions:{target:ts.ScriptTarget.ES2022}}).outputText;
function harness(overrides={}){
 const calls=[];const ctx={quitBusy:false,quitError:'',editing:false,running:0,roomSyncs:new Set(),taskList:[],editor:null,docs:[{id:'a',dirty:true,path:'/a.pdf',name:'a.pdf',format:'pdf'}],officeDrafts:{},rooms:{},officePending:()=>false,t:(_,en)=>en,$state:{snapshot:x=>x},save:async()=>'/copy.pdf',api:{save:async()=>calls.push('copy'),tasks:async()=>[],cancel:async()=>calls.push('cancel')},invoke:async command=>calls.push(command),...overrides};vm.createContext(ctx);vm.runInContext(source,ctx);return {ctx,calls};
}
test('quit saves before exiting and leaves window open on save failure',async()=>{const h=harness();await h.ctx.finishQuit('save');assert.deepEqual(h.calls,['save_original_document','quit_application']);const f=harness({invoke:async()=>{throw Error('disk full')}});await f.ctx.finishQuit('save');assert.match(f.ctx.quitError,/disk full/);assert.equal(f.ctx.docs[0].dirty,true);assert.equal(f.ctx.quitBusy,false);});
test('cancelled Save As never exits or marks a document saved',async()=>{const h=harness({save:async()=>null});await h.ctx.finishQuit('copy');assert.deepEqual(h.calls,[]);assert.equal(h.ctx.docs[0].dirty,true);});
test('a later save failure preserves earlier successful saves and does not exit',async()=>{const h=harness({docs:[{id:'a',dirty:true,path:'/a.pdf',format:'pdf'},{id:'b',dirty:true,path:'/b.pdf',format:'pdf'}],invoke:async(c,{id}={})=>{if(id==='b')throw Error('read only')}});await h.ctx.finishQuit('save');assert.equal(h.ctx.docs[0].dirty,false);assert.equal(h.ctx.docs[1].dirty,true);assert.match(h.ctx.quitError,/read only/);});
test('discard cancels outstanding work before exit',async()=>{const h=harness({running:1,taskList:[{id:'t',state:'running'}]});await h.ctx.finishQuit('discard');assert.deepEqual(h.calls,['cancel','quit_application']);});
