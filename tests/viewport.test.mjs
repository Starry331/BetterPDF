import {test} from 'node:test';import assert from 'node:assert/strict';import ts from 'typescript';import {readFileSync} from 'node:fs';
const js=ts.transpileModule(readFileSync(new URL('../src/lib/viewport.ts',import.meta.url),'utf8'),{compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.ES2022}}).outputText;
const {rowAt,visibleRange}=await import('data:text/javascript;base64,'+Buffer.from(js).toString('base64'));
const rows=Array.from({length:20000},(_,i)=>({y:i*858,height:842}));
test('120/144/240Hz traces keep mounted pages bounded and update only page boundaries',()=>{
 for(const hz of [60,120,144,240]){let previous='',changes=0;for(let frame=0;frame<hz*10;frame++){const top=frame/hz*1200;const r=visibleRange(rows,top,900,900);assert(r.end-r.start<=5);const current=rowAt(rows,top+180);assert(current>=r.start&&current<r.end);const key=JSON.stringify(r);if(key!==previous)changes++;previous=key;}assert(changes<40);}
});
test('jumps and variable page sizes include visible pages at both document edges',()=>{assert.deepEqual(visibleRange([],0,900,400),{start:0,end:0});assert.equal(rowAt(rows,Infinity),19999);assert.equal(visibleRange(rows,rows.at(-1).y,900,400).end,20000);const mixed=[{y:0,height:20},{y:36,height:2000},{y:2052,height:100}];assert.equal(rowAt(mixed,1000),1);assert.deepEqual(visibleRange(mixed,1000,400,100),{start:1,end:2});});
