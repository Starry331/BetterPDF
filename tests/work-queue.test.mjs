import {test} from 'node:test';import assert from 'node:assert/strict';
import ts from 'typescript';import {readFileSync} from 'node:fs';
const js=ts.transpileModule(readFileSync(new URL('../src/lib/work-queue.ts',import.meta.url),'utf8'),{compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.ES2022}}).outputText;
const {WorkQueue,BoundedCache}=await import('data:text/javascript;base64,'+Buffer.from(js).toString('base64'));
test('foreground overtakes thumbnails; cancelled work never starts; failures free slots',async()=>{
 const q=new WorkQueue(1),calls=[];let release;const first=q.run(()=>new Promise(r=>release=r));await Promise.resolve();
 const thumb=q.run(async()=>calls.push('thumbnail'),1);const abort=new AbortController();const cancelled=q.run(async()=>calls.push('cancelled'),0,abort.signal);const rejected=assert.rejects(cancelled,{name:'AbortError'});abort.abort();const foreground=q.run(async()=>calls.push('page'));release();await Promise.all([first,thumb,foreground,rejected]);assert.deepEqual(calls,['page','thumbnail']);await assert.rejects(q.run(async()=>{throw Error('failed')}));assert.equal(await q.run(async()=>42),42);
});
test('cache evicts least recently used pages under byte and count limits',()=>{const c=new BoundedCache(10,2);c.put('a:0',1,4);c.put('b:0',2,4);assert.equal(c.get('a:0'),1);c.put('c:0',3,4);assert.equal(c.get('b:0'),undefined);c.put('huge',9,11);assert.equal(c.get('huge'),undefined);c.clearPrefix('a:');assert.equal(c.get('a:0'),undefined);assert.equal(c.get('c:0'),3);});
