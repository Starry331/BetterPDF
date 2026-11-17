import {test} from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import ts from 'typescript';
import vm from 'node:vm';
const compile=s=>ts.transpileModule(s,{compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.ES2022}}).outputText.replace(/^import .*?;\s*/gm,'').replace(/export /g,'');
const queue=compile(readFileSync(new URL('../src/lib/work-queue.ts',import.meta.url),'utf8'));
const api=compile(readFileSync(new URL('../src/lib/api.ts',import.meta.url),'utf8'));
function harness(invoke){const context={window:{},invoke,convertFileSrc:x=>x,DOMException};vm.createContext(context);vm.runInContext(queue+'\n'+api+'\nglobalThis.testApi=api;',context);return context.testApi;}
test('light and glyph page data have separate cache entries',async()=>{const calls=[];const a=harness(async(_,args)=>{calls.push(args.includeChars);return {detail:args.includeChars}});assert.equal((await a.page('d',0,0,undefined,false)).detail,false);assert.equal((await a.page('d',0,0,undefined,true)).detail,true);await a.page('d',0,0,undefined,false);assert.deepEqual(calls,[false,true]);});
test('late results from an unmounted page do not refill its cache',async()=>{let resolve;let calls=0;const a=harness(()=>{calls++;return new Promise(r=>resolve=r)});const controller=new AbortController();const pending=a.page('d',0,0,controller.signal);await Promise.resolve();controller.abort();resolve({old:true});await pending;const next=a.page('d',0);await Promise.resolve();await Promise.resolve();assert.equal(calls,2);resolve({old:false});assert.equal((await next).old,false);});
