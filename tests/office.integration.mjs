import {spawnSync} from 'node:child_process';
import {mkdirSync,writeFileSync,readFileSync,statSync} from 'node:fs';
import {resolve,join} from 'node:path';
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
const root=resolve(import.meta.dirname,'..'),run=process.env.BPDF_TEST_LEGACY?JSON.parse(readFileSync(join(root,'work/office-integration-results.json'),'utf8')).run:join(root,'work','office-'+Date.now());mkdirSync(run,{recursive:true});
const cli=join(root,'src-tauri/target/debug/bpdf-cli'),runtime=join(root,'src-tauri/resources/runtime/bin');let seq=0;const results=[];
function request(req){const file=join(run,`request-${seq++}.json`);writeFileSync(file,JSON.stringify(req));const p=spawnSync(cli,[file],{encoding:'utf8',timeout:360000});const v=JSON.parse(p.stdout||'{}');if(!v.ok)throw Error(v.error||p.stderr||p.error);return v.result;}
function test(name,fn){const start=performance.now();try{fn();results.push({name,ok:true,ms:Math.round(performance.now()-start)});console.log('PASS',name);}catch(e){results.push({name,ok:false,error:String(e)});console.log('FAIL',name,String(e));}}
const sample=join(run,'sample.pdf');assert.equal(spawnSync(join(runtime,'mutool'),['run',join(root,'tests/fixtures.js'),sample]).status,0);
for(const fmt of (process.env.BPDF_TEST_LEGACY?[]:['docx','pptx','xlsx']))test(`${fmt}: native content editing, preview, save, reopen, undo/redo`,()=>{
 const input=join(run,`source.${fmt}`),output=join(run,`edited.${fmt}`),preview=join(run,`edited-${fmt}.pdf`);
 request({mode:'job',tool:'to_'+fmt,inputs:[sample],output:input,options:{}});
 const hash=createHash('sha256').update(readFileSync(input)).digest('hex');
 const before=request({mode:'office',input});assert.equal(before.document.format,fmt);assert(before.document.pages>0);
 const item=before.before.items.find(i=>i.editable);assert(item);
 const text=fmt==='xlsx'?'2468':'Edited 中文 & content';const kind=fmt==='xlsx'?'number':'text';
 const result=request({mode:'office',input,output,item:item.id,text,kind,style:fmt==='xlsx'?{}:{bold:true,underline:true,size:20},history:true,previewOutput:preview});
 assert.equal(result.after.items.find(i=>i.id===item.id).text,text);
 assert.equal(result.history[0].content.items.find(i=>i.id===item.id).text,item.text);
 assert.equal(result.history[1].content.items.find(i=>i.id===item.id).text,text);
 assert.equal(result.history[1].view.dirty,false);assert(statSync(output).size>100);
 const reopened=request({mode:'office',input:output});assert.equal(reopened.before.items.find(i=>i.id===item.id).text,text);
 assert.equal(createHash('sha256').update(readFileSync(input)).digest('hex'),hash);
 assert.throws(()=>request({mode:'office',input,output:input,item:item.id,text,kind}),/already exists/);
 const page=request({action:'page',input:preview,page:0});assert(page.lines.some(l=>l.text.includes(fmt==='xlsx'?'2468':'Edited')),'Preview contains edited content');
});
if(!process.env.BPDF_TEST_LEGACY)test('xlsx: local SUM formula recalculates in real preview',()=>{
 const input=join(run,'source.xlsx'),output=join(run,'formula.xlsx'),preview=join(run,'formula.pdf');
 const before=request({mode:'office',input});const item=before.before.items.find(i=>i.editable);const result=request({mode:'office',input,output,item:item.id,text:'=SUM(11,22)',kind:'formula',previewOutput:preview});
 assert.equal(result.after.items.find(i=>i.id===item.id).text,'=SUM(11,22)');const page=request({action:'page',input:preview,page:0});assert(page.lines.some(l=>l.text.includes('33')));
});
if(!process.env.BPDF_TEST_LEGACY)test('xlsx: insert addressed cell and verify recalculated preview',()=>{
 const input=join(run,'source.xlsx'),output=join(run,'inserted.xlsx');const before=request({mode:'office',input});const part=before.before.parts[0];const r=request({mode:'office',input,output,item:100000,text:'=SUM(10,20)',kind:'formula',style:{part,address:'Z100'}});const reopened=request({mode:'office',input:output});assert(reopened.before.total>before.before.total);assert(r.document.pages>0);
});
for(const [modern,legacy]of [['docx','doc'],['xlsx','xls'],['pptx','ppt']])test(`${legacy}: automatic modern-format compatibility mode`,()=>{
 const profile=new URL('file://'+join(run,`legacy-profile-${legacy}`)).href;
 const p=spawnSync(join(runtime,'../office/Contents/MacOS/soffice'),[`-env:UserInstallation=${profile}`,'--headless','--convert-to',legacy,'--outdir',run,join(run,`source.${modern}`)],{encoding:'utf8',timeout:120000});assert.equal(p.status,0,p.stderr);
 const result=request({mode:'office',input:join(run,`source.${legacy}`)});assert.equal(result.document.format,modern);assert(result.before.items.length>0);
});
writeFileSync(join(root,'work/office-integration-results.json'),JSON.stringify({run,results},null,2));if(results.some(r=>!r.ok))process.exit(1);
