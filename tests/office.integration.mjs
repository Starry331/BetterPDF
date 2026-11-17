import {spawnSync} from 'node:child_process';
import {mkdirSync,writeFileSync,readFileSync,statSync} from 'node:fs';
import {resolve,join} from 'node:path';
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
const root=resolve(import.meta.dirname,'..'),run=process.env.BPDF_TEST_LEGACY?JSON.parse(readFileSync(join(root,'work/office-integration-results.json'),'utf8')).run:join(root,'work','office-'+Date.now());mkdirSync(run,{recursive:true});
const cli=process.env.BPDF_TEST_CLI||join(root,'src-tauri/target/debug/bpdf-cli'),runtime=join(process.env.BPDF_RESOURCES||join(root,'src-tauri/resources'),'runtime/bin');let seq=0;const results=[];
function request(req){const file=join(run,`request-${seq++}.json`);writeFileSync(file,JSON.stringify(req));const p=spawnSync(cli,[file],{encoding:'utf8',timeout:360000});if(p.error||p.signal)throw Error(`Office worker stopped: ${p.error||p.signal}; ${p.stderr||''}`);let v;try{v=JSON.parse(p.stdout||'{}');}catch(e){throw Error(`Invalid worker response (${Buffer.byteLength(p.stdout||'')} bytes, exit ${p.status}): ${e.message}; ${p.stderr||''}`);}if(p.status!==0||!v.ok)throw Error(v.error||p.stderr||`Worker exited ${p.status}`);return v.result;}
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
if(!process.env.BPDF_TEST_LEGACY)for(const fmt of ['docx','pptx'])test(`${fmt}: multiline font and color survive reopen and real preview`,()=>{
 const input=join(run,`source.${fmt}`),output=join(run,`multiline.${fmt}`),preview=join(run,`multiline-${fmt}.pdf`);
 const before=request({mode:'office',input});const item=before.before.items.find(i=>i.editable);
 request({mode:'office',input,output,item:item.id,text:'First line\nSecond line',kind:'text',style:{font:'Liberation Sans',color:'#227744',size:14},previewOutput:preview});
 const reopened=request({mode:'office',input:output});assert(reopened.before.items.some(i=>i.text==='First line'));assert(reopened.before.items.some(i=>i.text==='Second line'));
 const result=request({action:'page',input:preview,page:0});const first=result.lines.find(l=>l.text.includes('First line')),second=result.lines.find(l=>l.text.includes('Second line'));assert(first&&second);assert(second.rect[1]>first.rect[1]);assert(Math.abs(first.color[1]-0x77/255)<.02);
});
if(!process.env.BPDF_TEST_LEGACY)test('docx: insert paragraph, preview and undo/redo',()=>{
 const input=join(run,'source.docx'),output=join(run,'paragraph.docx'),preview=join(run,'paragraph.pdf');const before=request({mode:'office',input});const item=before.before.items.find(i=>i.editable);
 const result=request({mode:'office',input,output,item:item.id,text:'Inserted paragraph',kind:'paragraph_after',history:true,previewOutput:preview});
 assert(result.after.items.some(i=>i.text==='Inserted paragraph'));assert(!result.history[0].content.items.some(i=>i.text==='Inserted paragraph'));assert(result.history[1].content.items.some(i=>i.text==='Inserted paragraph'));
 assert(request({action:'page',input:preview,page:0}).lines.some(l=>l.text.includes('Inserted paragraph')));
});
if(!process.env.BPDF_TEST_LEGACY)test('pptx: reorder slides preserves objects, preview and history',()=>{
 const input=join(run,'source.pptx'),output=join(run,'reordered.pptx'),preview=join(run,'reordered.pdf'),baseline=join(run,'slides-before.pdf');
 const before=request({mode:'office',input,previewOutput:baseline});assert(before.before.parts.length>=2);const part=before.before.parts[1];
 const result=request({mode:'office',input,output,item:100001,text:'',kind:'slide_up',style:{part},history:true,previewOutput:preview});
 assert.equal(result.after.parts[0],part);assert.equal(result.history[0].content.parts[0],before.before.parts[0]);assert.equal(result.history[1].content.parts[0],part);
 assert.deepEqual(request({action:'page',input:preview,page:0}).lines.map(l=>l.text),request({action:'page',input:baseline,page:1}).lines.map(l=>l.text));
});
if(!process.env.BPDF_TEST_LEGACY)test('pptx: duplicate and delete slides survive preview, reopen and history',()=>{
 const input=join(run,'with-picture.pptx'),copy=join(run,'copied.pptx'),output=join(run,'deleted.pptx'),preview=join(run,'copied.pdf');
 const picture=join(run,'picture.png'),picturePdf=join(run,'with-picture.pdf');
 request({action:'render',input:sample,page:0,scale:.25,output:picture});request({action:'edit',input:sample,output:picturePdf,edit:{kind:'image',insert:true,page:0,rect:[400,400,470,500],path:picture}});
 request({mode:'job',tool:'to_pptx',inputs:[picturePdf],output:input,options:{}});
 const before=request({mode:'office',input});const part=before.before.parts[0];
 const result=request({mode:'office',input,output:copy,item:100001,text:'',kind:'slide_duplicate',style:{part},history:true,previewOutput:preview});
 assert.equal(result.document.pages,before.document.pages+1);assert.equal(result.after.parts.length,before.before.parts.length+1);assert.notEqual(result.after.parts[1],part);
 assert.equal(result.history[0].content.parts.length,before.before.parts.length);assert.equal(result.history[1].content.parts.length,before.before.parts.length+1);
 assert.deepEqual(request({action:'page',input:preview,page:0}).lines.map(l=>l.text),request({action:'page',input:preview,page:1}).lines.map(l=>l.text));
 const left=request({action:'page',input:preview,page:0}),right=request({action:'page',input:preview,page:1});assert(left.images.length>0);assert.deepEqual(left.images.map(i=>i.rect),right.images.map(i=>i.rect));
 const reopened=request({mode:'office',input:copy});assert.equal(reopened.before.parts.length,result.after.parts.length);
 const deleted=request({mode:'office',input:copy,output,item:100001,text:'',kind:'slide_delete',style:{part:result.after.parts[1]},history:true});
 assert.equal(deleted.document.pages,before.document.pages);assert.deepEqual(deleted.after.parts,before.before.parts);assert.equal(deleted.history[0].content.parts.length,before.before.parts.length+1);assert.equal(deleted.history[1].content.parts.length,before.before.parts.length);
 assert.equal(request({mode:'office',input:output}).document.pages,before.document.pages);
});
if(!process.env.BPDF_TEST_LEGACY)test('pptx: blank slide and independent editable text box survive preview and history',()=>{
 const input=join(run,'source.pptx'),blank=join(run,'blank.pptx'),output=join(run,'textbox.pptx'),preview=join(run,'blank.pdf'),textPreview=join(run,'textbox.pdf');const before=request({mode:'office',input});const part=before.before.parts[0];
 const result=request({mode:'office',input,output:blank,item:100001,text:'',kind:'slide_blank',style:{part},history:true,previewOutput:preview});const added=result.after.parts[1];
 assert.equal(result.document.pages,before.document.pages+1);assert.equal(request({action:'page',input:preview,page:1}).lines.length,0);assert.equal(request({action:'page',input:preview,page:1}).images.length,0);assert.equal(result.history[0].content.parts.length,before.before.parts.length);
 const edited=request({mode:'office',input:blank,output,item:100002,text:'New editable text\nSecond line',kind:'slide_text',style:{part:added},history:true,previewOutput:textPreview});
 assert(edited.after.items.some(x=>x.part===added&&x.text==='New editable text'));assert(!edited.history[0].content.items.some(x=>x.part===added));assert(request({action:'page',input:textPreview,page:1}).lines.some(x=>x.text==='New editable text'));
 assert.deepEqual(edited.after.items.filter(x=>x.part===part).map(x=>x.text),before.before.items.filter(x=>x.part===part).map(x=>x.text));assert(request({mode:'office',input:output}).before.items.some(x=>x.part===added&&x.text==='Second line'));
});
if(!process.env.BPDF_TEST_LEGACY)test('docx: paragraph alignment changes real layout and supports undo',()=>{
 const input=join(run,'source.docx'),output=join(run,'aligned.docx'),preview=join(run,'aligned.pdf'),original=join(run,'before-align.pdf');const before=request({mode:'office',input,previewOutput:original});const item=before.before.items.find(i=>i.editable);
 const result=request({mode:'office',input,output,item:item.id,text:'',kind:'paragraph_align',style:{alignment:'center'},history:true,previewOutput:preview});assert.deepEqual(result.after.items.map(i=>i.text),before.before.items.map(i=>i.text));
 const first=request({action:'page',input:original,page:0}).lines.find(l=>l.text.includes(item.text.trim())),aligned=request({action:'page',input:preview,page:0}).lines.find(l=>l.text.includes(item.text.trim()));assert(first&&aligned);assert(aligned.rect[0]>first.rect[0]+20);
 assert.equal(result.history.length,2);assert.deepEqual(request({mode:'office',input:output}).before.items.map(i=>i.text),before.before.items.map(i=>i.text));
});
if(!process.env.BPDF_TEST_LEGACY)test('docx: delete inserted paragraph, reopen and undo/redo',()=>{
 const input=join(run,'paragraph.docx'),output=join(run,'paragraph-deleted.docx'),preview=join(run,'paragraph-deleted.pdf');const before=request({mode:'office',input});const item=before.before.items.find(i=>i.text==='Inserted paragraph');assert(item);
 const result=request({mode:'office',input,output,item:item.id,text:'',kind:'paragraph_delete',history:true,previewOutput:preview});assert(!result.after.items.some(i=>i.text==='Inserted paragraph'));assert(result.history[0].content.items.some(i=>i.text==='Inserted paragraph'));assert(!result.history[1].content.items.some(i=>i.text==='Inserted paragraph'));
 assert(!request({action:'page',input:preview,page:0}).lines.some(l=>l.text.includes('Inserted paragraph')));assert(!request({mode:'office',input:output}).before.items.some(i=>i.text==='Inserted paragraph'));
});
if(!process.env.BPDF_TEST_LEGACY)test('docx: numbered list continues on insertion and can be removed',()=>{
 const input=join(run,'source.docx'),numbered=join(run,'numbered.docx'),continued=join(run,'continued.docx'),removed=join(run,'unlisted.docx'),preview=join(run,'list.pdf');const before=request({mode:'office',input});const item=before.before.items.find(x=>x.editable);
 const first=request({mode:'office',input,output:numbered,item:item.id,text:'',kind:'paragraph_list',style:{list:'decimal'},history:true});assert.deepEqual(first.after.items.map(x=>x.text),before.before.items.map(x=>x.text));
 request({mode:'office',input:numbered,output:continued,item:item.id,text:'Second list item',kind:'paragraph_after',previewOutput:preview});
 const lines=request({action:'page',input:preview,page:0}).lines.map(x=>x.text.trim());assert(lines.some(x=>x==='1.'||x.startsWith('1. ')),JSON.stringify(lines));assert(lines.some(x=>x==='2.'||x.startsWith('2. ')),JSON.stringify(lines));
 const content=request({mode:'office',input:continued});const second=content.before.items.find(x=>x.text==='Second list item');assert(second);
 const result=request({mode:'office',input:continued,output:removed,item:second.id,text:'',kind:'paragraph_list',style:{list:'none'},history:true,previewOutput:join(run,'unlisted.pdf')});assert(result.after.items.some(x=>x.text==='Second list item'));assert(!request({action:'page',input:join(run,'unlisted.pdf'),page:0}).lines.some(x=>x.text.trim()==='2.'));assert(request({mode:'office',input:removed}).before.items.some(x=>x.text==='Second list item'));
 const bullet=join(run,'bullet.docx');request({mode:'office',input,output:bullet,item:item.id,text:'',kind:'paragraph_list',style:{list:'bullet'},previewOutput:join(run,'bullet.pdf')});assert(request({action:'page',input:join(run,'bullet.pdf'),page:0}).lines.some(x=>x.text.includes('•')));
});
if(!process.env.BPDF_TEST_LEGACY)test('xlsx: isolated cell styling and numeric format survive real preview and reopen',()=>{
 const input=join(run,'source.xlsx'),output=join(run,'styled.xlsx'),preview=join(run,'styled.pdf');const before=request({mode:'office',input});const item=before.before.items.find(x=>x.editable);
 const result=request({mode:'office',input,output,item:item.id,text:'0.125',kind:'number',style:{bold:true,size:16,font:'Liberation Sans',color:'#227744',fill:'#FFF2CC',horizontal:'center',wrap:true,columnWidth:24,rowHeight:30,numberFormat:'percent'},history:true,previewOutput:preview});
 const line=request({action:'page',input:preview,page:0}).lines.find(x=>x.text.includes('12.50%'));assert(line);assert(Math.abs(line.color[1]-0x77/255)<.02);assert.deepEqual(result.after.items.filter(x=>x.id!==item.id).map(x=>x.text),before.before.items.filter(x=>x.id!==item.id).map(x=>x.text));assert.equal(result.history[0].content.items.find(x=>x.id===item.id).text,item.text);assert.equal(request({mode:'office',input:output}).before.items.find(x=>x.id===item.id).text,'0.125');
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
