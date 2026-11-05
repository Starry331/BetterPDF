import {spawnSync} from 'node:child_process';
import {mkdirSync,writeFileSync,readFileSync,existsSync,readdirSync,statSync} from 'node:fs';
import {resolve,join} from 'node:path';import assert from 'node:assert/strict';import {createHash} from 'node:crypto';
const root=resolve(import.meta.dirname,'..'),run=join(root,'work','integration-'+Date.now());mkdirSync(run,{recursive:true});
const cli=join(root,'src-tauri/target/debug/bpdf-cli'),mutool=join(root,'src-tauri/resources/runtime/bin/mutool');let seq=0;const results=[];
function request(req){const f=join(run,'request-'+seq+++'.json');writeFileSync(f,JSON.stringify(req));const start=performance.now();const p=spawnSync(cli,[f],{encoding:'utf8',timeout:180000,cwd:root});let parsed;try{parsed=JSON.parse(p.stdout.trim());}catch{throw Error(p.stderr||p.error||p.stdout)}if(!parsed.ok)throw Error(parsed.error);return {data:parsed.result,ms:performance.now()-start};}
function job(tool,inputs,options={},suffix='pdf'){const output=join(run,tool+'-'+seq+(suffix?'.'+suffix:''));return {...request({mode:'job',tool,inputs,output,options}),output};}
function info(path,password=''){return request({action:'info',input:path,password}).data;}
function page(path,p=0){return request({action:'page',input:path,page:p}).data;}
function test(name,fn){const start=performance.now();try{fn();results.push({name,ok:true,ms:Math.round(performance.now()-start)});console.log('PASS',name);}catch(e){results.push({name,ok:false,error:String(e)});console.log('FAIL',name,String(e));}}
const sample=join(run,'sample.pdf');assert.equal(spawnSync(mutool,['run',join(root,'tests/fixtures.js'),sample]).status,0);const originalHash=createHash('sha256').update(readFileSync(sample)).digest('hex');
const simple=join(run,'simple.pdf');request({action:'fixture',output:simple,pages:3});
let png=join(run,'sample.png');request({action:'render',input:sample,page:0,scale:1,output:png});let docx,pptx,xlsx,epub,protectedFile,watermarked;
test('native rendering and text extraction',()=>{assert(statSync(png).size>1000);assert(page(sample).lines.some(l=>l.text.includes('Original editable sentence')));});
test('merge',()=>assert.equal(info(job('merge',[sample,simple]).output).pages,6));
test('split',()=>{const r=job('split',[sample],{chunk:2},'');assert.equal(readdirSync(r.output).length,2);});
test('rotate',()=>{const r=job('rotate',[sample],{pages:'2',angle:90});assert.equal(info(r.output).bounds[1][2],842);});
test('delete',()=>assert.equal(info(job('delete',[sample],{pages:'2'}).output).pages,2));
test('extract and descending ranges',()=>assert.equal(info(job('extract',[sample],{pages:'3-1'}).output).pages,3));
test('reorder with duplicates',()=>assert.equal(info(job('reorder',[sample],{pages:'3,1,1,2'}).output).pages,4));
test('resize',()=>assert.equal(info(job('resize',[simple],{width:420,height:595}).output).bounds[0][2],420));
test('crop',()=>assert.equal(info(job('crop',[sample],{left:10,right:10,top:20,bottom:20}).output).bounds[0][2],575));
test('PDF to DOCX',()=>{docx=job('to_docx',[sample],{},'docx').output;assert(statSync(docx).size>1000);});
test('PDF to PPTX',()=>{pptx=job('to_pptx',[sample],{},'pptx').output;assert(statSync(pptx).size>1000);});
test('PDF to XLSX',()=>{xlsx=job('to_xlsx',[sample],{},'xlsx').output;assert(statSync(xlsx).size>1000);});
test('PDF to images',()=>{const r=job('to_images',[sample],{pages:'2-3',dpi:72},'');assert.equal(readdirSync(r.output).length,2);});
test('PDF/A-2b generation',()=>assert.equal(info(job('to_pdfa',[sample]).output).pages,3));
test('PDF to TXT',()=>assert(readFileSync(job('to_txt',[sample],{},'txt').output,'utf8').includes('Notebook')));
test('PDF to Markdown',()=>assert(readFileSync(job('to_md',[sample],{},'md').output,'utf8').includes('Notebook')));
test('PDF to EPUB',()=>{epub=job('to_epub',[sample],{},'epub').output;assert(statSync(epub).size>1000);});
test('Word to PDF roundtrip',()=>{assert(docx);const r=job('word_pdf',[docx]);assert(info(r.output).pages>=3);assert(page(r.output).lines.some(l=>l.text.includes('Original editable sentence')));});
test('PowerPoint to PDF roundtrip',()=>{assert(pptx);const r=job('ppt_pdf',[pptx]);assert.equal(info(r.output).pages,3);assert(page(r.output).lines.some(l=>l.text.includes('Notebook')));});
test('Excel to PDF roundtrip',()=>{assert(xlsx);const r=job('excel_pdf',[xlsx]);assert(info(r.output).pages>=1);});
test('images to PDF',()=>assert.equal(info(job('images_pdf',[png,png]).output).pages,2));
test('EPUB to PDF',()=>{assert(epub);assert(info(job('epub_pdf',[epub]).output).pages>=1);});
test('CAJ recovery',()=>{const pdf=readFileSync(simple);const header=Buffer.alloc(512);header.write('CAJ');header.writeUInt32LE(3,0x10);header.writeUInt32LE(0x20,0x14);header.writeUInt32LE(512,0x20);const caj=join(run,'test.caj');writeFileSync(caj,Buffer.concat([header,pdf]));assert.equal(info(job('caj_pdf',[caj]).output).pages,3);});
test('generic file to PDF',()=>{const file=join(run,'hello.txt');writeFileSync(file,'Hello BPDF\nLocal text document.');assert(info(job('file_pdf',[file]).output).pages>=1);});
test('lossless compression',()=>assert.equal(info(job('compress',[sample],{quality:'lossless'}).output).pages,3));
test('repair',()=>assert.equal(info(job('repair',[sample]).output).pages,3));
test('OCR with bundled English model',()=>{const r=job('ocr',[simple],{dpi:144,language:'eng'});assert.equal(info(r.output).pages,3);assert(page(r.output).lines.some(l=>l.text.toLowerCase().includes('bpdf')));});
test('grayscale',()=>assert.equal(info(job('grayscale',[sample],{dpi:72,colorMode:'gray'}).output).pages,3));
test('add watermark',()=>{watermarked=job('watermark',[sample],{text:'WATERMARK',size:24}).output;assert(page(watermarked).lines.some(l=>l.text.includes('WATERMARK')));});
test('remove independent watermark',()=>{assert(watermarked);const r=job('remove_watermark',[watermarked],{watermarkText:'WATERMARK'});assert(!page(r.output).lines.some(l=>l.text.includes('WATERMARK')));assert(page(r.output).lines.some(l=>l.text.includes('Notebook')));});
test('AES-256 protection',()=>{protectedFile=job('protect',[sample],{newPassword:'local-pass-123'}).output;assert.throws(()=>info(protectedFile),/PASSWORD_REQUIRED/);assert.equal(info(protectedFile,'local-pass-123').pages,3);});
test('unlock',()=>{assert(protectedFile);assert.equal(info(job('unlock',[protectedFile],{password:'local-pass-123'}).output).pages,3);});
test('rasterize',()=>{const r=job('rasterize',[sample],{dpi:72});assert.equal(info(r.output).pages,3);assert.equal(page(r.output).lines.length,0);});
test('flatten fields',()=>{const r=job('flatten',[sample]);assert.equal(page(r.output).widgets.length,0);assert(page(r.output).lines.some(l=>l.text.includes('Alice')));});
test('generic file to images',()=>{assert(docx);const r=job('file_images',[docx],{dpi:72},'');assert(readdirSync(r.output).length>=3);});
test('real text edit removes original glyphs',()=>{const l=page(sample).lines.find(l=>l.text.includes('Original editable'));const output=join(run,'edited.pdf');request({action:'edit',input:sample,output,edit:{kind:'text',page:0,rect:l.rect,targetRect:[50,94,450,125],size:12,text:'Replaced text, not covered.'}});const lines=page(output).lines;assert(!lines.some(l=>l.text.includes('Original editable')));assert(lines.some(l=>l.text.includes('Replaced text')));assert(page(output,1).lines.some(l=>l.text.includes('Original editable')));});
test('Chinese font edit and extraction',()=>{const output=join(run,'chinese.pdf');request({action:'edit',input:simple,output,edit:{kind:'text',insert:true,page:0,rect:[50,150,500,200],size:20,font:'zh-Hans',text:'轻量 PDF 工具 · 本地处理'}});assert(page(output).lines.some(l=>l.text.includes('本地处理')));});
test('form field value roundtrip',()=>{const output=join(run,'form-filled.pdf');request({action:'edit',input:sample,output,edit:{kind:'widget',page:0,widget:0,value:'Bob'}});assert.equal(page(output).widgets[0].value,'Bob');});
test('highlight / note / ink annotations',()=>{for(const e of [{kind:'highlight',page:0,rect:[50,94,192,111]},{kind:'note',page:0,rect:[400,100,424,124],text:'Review this'},{kind:'ink',page:0,points:[[50,400],[70,420],[100,405]]}]){const output=join(run,e.kind+'.pdf');request({action:'edit',input:sample,output,edit:e});assert.equal(info(output).pages,3);}});
test('reject invalid ranges, unknown CAJ and clobber',()=>{assert.throws(()=>job('delete',[sample],{pages:'0'}));assert.throws(()=>request({mode:'job',tool:'rotate',inputs:[sample],output:sample,options:{angle:90}}),/already exists/);const unknown=join(run,'unknown.caj');writeFileSync(unknown,'not a supported document');assert.throws(()=>job('caj_pdf',[unknown]),/Unsupported/);});
test('image move, replacement and deletion preserve other pages',()=>{const input=job('images_pdf',[png,png]).output;const before=page(input).images[0];const moved=join(run,'image-moved.pdf');request({action:'edit',input,output:moved,edit:{kind:'image',page:0,rect:before.rect,targetRect:[20,30,220,330]}});assert.deepEqual(page(moved).images[0].rect,[20,30,220,330]);assert.deepEqual(page(moved,1).images[0].rect,before.rect);const replaced=join(run,'image-replaced.pdf');request({action:'edit',input:moved,output:replaced,edit:{kind:'image',page:0,rect:[20,30,220,330],path:png,targetRect:[40,50,240,350]}});assert.deepEqual(page(replaced).images[0].rect,[40,50,240,350]);const deleted=join(run,'image-deleted.pdf');request({action:'edit',input:replaced,output:deleted,edit:{kind:'image',page:0,rect:[40,50,240,350],remove:true}});assert.equal(page(deleted).images.length,0);assert.equal(page(deleted,1).images.length,1);});
test('font synchronization preserves font, size and color',()=>{const input=join(run,'font-source.pdf');assert.equal(spawnSync(mutool,['run',join(root,'tests/font-fixture.js'),input]).status,0);const l=page(input).lines[0],output=join(run,'font-synced.pdf');const color=l.color;request({action:'edit',input,output,edit:{kind:'text',page:0,rect:l.rect,targetRect:[50,120,500,170],text:'Styled text edited',font:'__original__',size:l.size,color}});const next=page(output).lines[0];assert.equal(next.text,'Styled text edited');assert.equal(next.font.replace(/^[A-Z]{6}\+/,''),'NimbusRoman-BoldItalic');assert.equal(next.size,l.size);assert.deepEqual(next.color,l.color);assert.throws(()=>request({action:'edit',input,output:join(run,'unsupported-font.pdf'),edit:{kind:'text',page:0,rect:l.rect,targetRect:[50,120,500,170],text:'中文',font:'__original__',size:l.size}}),/FONT_SYNC/);});
test('original is byte-for-byte unchanged',()=>assert.equal(createHash('sha256').update(readFileSync(sample)).digest('hex'),originalHash));
writeFileSync(join(root,'work/integration-results.json'),JSON.stringify({run,results,passed:results.filter(r=>r.ok).length,total:results.length},null,2));console.log(`\n${results.filter(r=>r.ok).length}/${results.length} passed; artifacts: ${run}`);process.exitCode=results.some(r=>!r.ok)?1:0;
