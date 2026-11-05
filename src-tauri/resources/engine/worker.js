/* SPDX-License-Identifier: AGPL-3.0-only
 * Trusted host script for the native MuPDF interpreter. Never evaluates document JS.
 * Requests arrive through a private file; passwords never appear in process arguments.
 */
'use strict';
function fail(message) { throw new Error(message); }
function number(v, fallback, min, max) { var n=v===undefined?fallback:Number(v); if(!isFinite(n)||n<min||n>max) fail('Invalid numeric option'); return n; }
function rect(v) { if(!v||v.length!==4) fail('Invalid rectangle'); for(var i=0;i<4;i++) if(!isFinite(v[i])) fail('Invalid rectangle'); if(v[2]<=v[0]||v[3]<=v[1]) fail('Empty rectangle'); return v; }
function open(path,password) { var d=Document.openDocument(path); if(d.needsPassword()&&!d.authenticatePassword(password||'')) fail('PASSWORD_REQUIRED'); return d; }
function pages(spec,count,emptyAll) {
 if(!spec||spec==='all') { if(emptyAll===false) fail('Select at least one page'); var all=[];for(var i=0;i<count;i++)all.push(i);return all; }
 var result=[],parts=String(spec).split(',');
 for(var j=0;j<parts.length;j++){var p=parts[j].replace(/\s/g,'');if(!/^\d+(?:-\d+)?$/.test(p))fail('Invalid page range');var s=p.split('-'),a=+s[0],b=s.length===2?+s[1]:a;if(a<1||b<1||a>count||b>count)fail('Page out of range');var step=a<=b?1:-1;for(var k=a;;k+=step){result.push(k-1);if(k===b)break;}if(result.length>20000)fail('Too many pages');}
 return result;
}
function cleanActions(d) {
 var root=d.getTrailer().get('Root'); root.delete('OpenAction');root.delete('AA');var names=root.get('Names');if(names)names.delete('JavaScript');
 for(var i=0;i<d.countPages();i++){var p=d.findPage(i);p.delete('AA');var aa=p.get('Annots');if(aa)for(var j=0;j<aa.length;j++){aa.get(j).delete('AA');var a=aa.get(j).get('A');if(a&&a.get('S').asName()==='JavaScript')aa.get(j).delete('A');}}
}
function save(d,path,options){cleanActions(d);d.save(path,options||'garbage=4,compress,compress-fonts,compress-images');}
function info(d){if(d.countPages()>20000)fail('Document exceeds the 20,000-page safety limit');var bounds=[];for(var i=0;i<d.countPages();i++)bounds.push(d.loadPage(i).getBounds());return {pages:d.countPages(),bounds:bounds,outline:d.loadOutline()||[]};}
function data(d,p,assetDir){
 var page=d.loadPage(p), lines=[], images=[], current=null, block=0;
 page.toStructuredText('preserve-spans,preserve-images').walk({
  beginTextBlock:function(){block++;},
  beginLine:function(b,w,dir){current={text:'',rect:b,size:12,font:'',color:0,dir:dir,block:block};},
  onChar:function(c,origin,font,size,q,color){if(!current)return;current.text+=c;current.size=size;current.font=font.getName();current.color=Array.isArray(color)?color:[((color>>>16)&255)/255,((color>>>8)&255)/255,(color&255)/255];},
  endLine:function(){if(current&&current.text)lines.push(current);current=null;},
  onImageBlock:function(b,transform,image){var entry={id:images.length,rect:b};if(assetDir){entry.path=assetDir+'/image-'+p+'-'+images.length+'.png';image.toPixmap().saveAsPNG(entry.path);}images.push(entry);}
 });
 var widgets=[];if(page.getWidgets){var ws=page.getWidgets();for(var j=0;j<ws.length;j++){var w=ws[j];widgets.push({id:j,name:w.getName(),type:w.getFieldType(),value:w.getValue(),rect:w.getBounds(),options:(w.getFieldType()==='combobox'||w.getFieldType()==='listbox')?w.getOptions():[]});}}
 return {bounds:page.getBounds(),lines:lines,images:images,widgets:widgets};
}
function dictCopy(d,obj){var out=d.newDictionary();if(obj)obj.forEach(function(v,k){out.put(k,v);});return out;}
function resources(d,page){var obj=page.getObject(),rs=dictCopy(d,obj.getInheritable('Resources'));obj.put('Resources',rs);return rs;}
function append(d,page,content){var obj=page.getObject(),old=obj.get('Contents'),arr=d.newArray();arr.push(d.addStream('q'));if(old&&old.isArray())for(var i=0;i<old.length;i++)arr.push(old.get(i));else if(old)arr.push(old);arr.push(d.addStream('Q\n'+content));obj.put('Contents',arr);}
function textContent(d,page,text,box,size,color,opacity,fontName,originalFont){
 var font=originalFont||new Font(fontName||(/[^\x00-\x7f]/.test(text)?'zh-Hans':'Helvetica'));
 var rows=[],row='',width=0,max=box[2]-box[0];
 for(var i=0;i<text.length;i++){var c=text.charAt(i),code=text.charCodeAt(i),gid=font.encodeCharacter(code),adv=font.advanceGlyph(gid)*size;if(c==='\n'){rows.push(row);row='';width=0;continue;}if(!gid&&c!==' ')fail(originalFont?'FONT_SYNC: missing glyph':'Font does not contain a required character');if(width+adv>max&&row){rows.push(row);row='';width=0;}row+=c;width+=adv;}
 rows.push(row);if(rows.length*size*1.2>box[3]-box[1]+size*.25)fail('TEXT_OVERFLOW');
 var rs=resources(d,page),fonts=dictCopy(d,rs.get('Font')),key='BPDFText'+d.countObjects();var fontObj=d.addObject(dictCopy(d,d.addFont(font))),map={},pairs=[];
 for(var mi=0;mi<text.length;mi++){var unicode=text.charCodeAt(mi),glyph=font.encodeCharacter(unicode);map[('0000'+glyph.toString(16)).slice(-4)]=('0000'+unicode.toString(16)).slice(-4);}
 for(var mg in map)pairs.push('<'+mg+'> <'+map[mg]+'>');
 var cmap='/CIDInit /ProcSet findresource begin 12 dict begin begincmap /CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def /CMapName /BPDFUnicode def /CMapType 2 def 1 begincodespacerange <0000> <FFFF> endcodespacerange\n';
 for(var off=0;off<pairs.length;off+=100){var chunk=pairs.slice(off,off+100);cmap+=chunk.length+' beginbfchar\n'+chunk.join('\n')+'\nendbfchar\n';}
 cmap+='endcmap CMapName currentdict /CMap defineresource pop end end';fontObj.put('ToUnicode',d.addStream(cmap));fonts.put(key,fontObj);rs.put('Font',fonts);
 var gs=dictCopy(d,rs.get('ExtGState')),gkey='BPDFAlpha'+d.countObjects();gs.put(gkey,{Type:'ExtGState',ca:opacity===undefined?1:opacity,CA:opacity===undefined?1:opacity});rs.put('ExtGState',gs);
 var transform=Matrix.invert(page.getTransform()),out='q '+transform.join(' ')+' cm /'+gkey+' gs '+(color||[0,0,0]).join(' ')+' rg\n';
 for(var n=0;n<rows.length;n++){var hex='';for(var h=0;h<rows[n].length;h++)hex+=('0000'+font.encodeCharacter(rows[n].charCodeAt(h)).toString(16)).slice(-4);out+='BT /'+key+' '+size+' Tf 1 0 0 -1 '+box[0]+' '+(box[1]+size+n*size*1.2)+' Tm <'+hex+'> Tj ET\n';}
 return out+'Q';
}
function removeText(page,box){var a=page.createAnnotation('Redact');a.setRect([box[0]+.2,box[1]+.2,box[2]-.2,box[3]-.2]);page.applyRedactions(false,0,0,0);}
function imageContent(d,page,path,box){var image=typeof path==='string'?new Image(path):path,rs=resources(d,page),imgs=dictCopy(d,rs.get('XObject')),key='BPDFImage'+d.countObjects();imgs.put(key,d.addImage(image));rs.put('XObject',imgs);return 'q '+Matrix.invert(page.getTransform()).join(' ')+' cm '+(box[2]-box[0])+' 0 0 '+(-(box[3]-box[1]))+' '+box[0]+' '+box[3]+' cm /'+key+' Do Q';}
function edit(d,e){
 if(!Number.isInteger)Number.isInteger=function(n){return typeof n==='number'&&n%1===0;};
 var p=number(e.page,0,0,d.countPages()-1),page=d.loadPage(p),box=e.rect?rect(e.rect):null;
 if(e.kind==='text'){
  if(!box)fail('Select text to edit');var size=number(e.size,12,4,200),target=rect(e.targetRect||box);
  var originalFont=null;if(e.font==='__original__'){var foundFonts={},matched=false;page.toStructuredText('preserve-spans').walk({beginLine:function(b){matched=Math.abs(b[0]-box[0])<1&&Math.abs(b[1]-box[1])<1&&Math.abs(b[2]-box[2])<1&&Math.abs(b[3]-box[3])<1;},onChar:function(c,o,font){if(matched){originalFont=font;foundFonts[font.getName()]=true;}}});var fontCount=0;for(var fk in foundFonts)fontCount++;if(!originalFont||fontCount!==1)fail('FONT_SYNC: ambiguous original font');}
  if(!e.insert)removeText(page,box);var content=textContent(d,page,String(e.text||''),target,size,e.color||[0,0,0],1,e.font==='__original__'?null:e.font,originalFont);append(d,page,content);
 }else if(e.kind==='image'){
  if(!box)fail('Select an image');var im=data(d,p).images,overlaps=0;for(var ii=0;ii<im.length;ii++){var r=im[ii].rect;if(r[0]<box[2]-.5&&r[2]>box[0]+.5&&r[1]<box[3]-.5&&r[3]>box[1]+.5)overlaps++;}if(overlaps>1)fail('Overlapping images cannot be safely isolated');
  var source=e.path||null;if(!e.remove&&!source){page.toStructuredText('preserve-images').walk({onImageBlock:function(b,t,image){if(Math.abs(b[0]-box[0])<1&&Math.abs(b[1]-box[1])<1&&Math.abs(b[2]-box[2])<1&&Math.abs(b[3]-box[3])<1)source=image;}});if(!source)fail('Selected image no longer exists');}
  if(!e.insert){var ia=page.createAnnotation('Redact');ia.setRect([box[0]+.5,box[1]+.5,box[2]-.5,box[3]-.5]);page.applyRedactions(false,1,0,1);}if(!e.remove&&source)append(d,page,imageContent(d,page,source,rect(e.targetRect||box)));
 }else if(e.kind==='highlight'){var a=page.createAnnotation('Highlight');a.setQuadPoints(e.quads||[[box[0],box[1],box[2],box[1],box[0],box[3],box[2],box[3]]]);a.setColor(e.color||[1,.82,.2]);a.setOpacity(.45);a.update();
 }else if(e.kind==='note'){var n=page.createAnnotation('Text');n.setRect(box);n.setContents(String(e.text||''));n.setColor([.23,.45,.85]);n.update();
 }else if(e.kind==='ink'){var ink=page.createAnnotation('Ink');ink.setInkList([e.points]);ink.setColor(e.color||[.15,.35,.8]);ink.setBorderWidth(2);ink.update();
 }else if(e.kind==='widget'){var ws=page.getWidgets(),w=ws[e.widget];if(!w||w.isReadOnly())fail('Field is not editable');if(w.getFieldType()==='checkbox'||w.getFieldType()==='radiobutton'){if((w.getValue()!=='Off')!==!!e.checked)w.toggle();}else if((w.getFieldType()==='combobox'||w.getFieldType()==='listbox'))w.setChoiceValue(String(e.value));else if(w.getFieldType()==='text')w.setTextValue(String(e.value));else fail('Unsupported field type');page.update();
 }else fail('Unknown edit operation');
}
function checkedPixmap(page,scale,gray){var b=page.getBounds(),pixels=(b[2]-b[0])*(b[3]-b[1])*scale*scale;if(!isFinite(pixels)||pixels>40000000)fail('Page exceeds render memory limit; reduce DPI');return page.toPixmap([scale,0,0,scale,0,0],gray?ColorSpace.DeviceGray:ColorSpace.DeviceRGB,false,true);}
function raster(d,selected,r,gray){var out=new PDFDocument();for(var i=0;i<selected.length;i++){var p=d.loadPage(selected[i]),b=p.getBounds(),scale=number(r.dpi,144,36,600)/72,pm=checkedPixmap(p,scale,gray),img=out.addImage(new Image(pm)),w=b[2]-b[0],h=b[3]-b[1],obj=out.addPage([0,0,w,h],0,{XObject:{Im:img}},'q '+w+' 0 0 '+h+' 0 0 cm /Im Do Q');out.insertPage(-1,obj);}return out;}
function run(r){
 var o=r.options||{},d,i,p,selected;
 if(r.action==='fixture'){d=new PDFDocument();for(i=0;i<(r.pages||3);i++){var f=d.addSimpleFont(new Font('Helvetica'));d.insertPage(-1,d.addPage([0,0,595,842],0,{Font:{F:f}},'BT /F 24 Tf 50 770 Td (BPDF Test '+(i+1)+') Tj /F 12 Tf 0 -35 Td (Local documents. Real editable text.) Tj ET'));}save(d,r.output);return info(d);}
 if(r.action==='images_pdf'){d=new PDFDocument();for(i=0;i<r.inputs.length;i++){var im=new Image(r.inputs[i]),w=im.getWidth(),h=im.getHeight(),scale=Math.min(1,1440/Math.max(w,h));w*=scale;h*=scale;d.insertPage(-1,d.addPage([0,0,w,h],0,{XObject:{Im:d.addImage(im)}},'q '+w+' 0 0 '+h+' 0 0 cm /Im Do Q'));}save(d,r.output);return {};}
 d=open(r.input||r.inputs[0],r.password||o.password);
 if(r.action==='info')return info(d);
 if(r.action==='page')return data(d,r.page,r.assetDir);
 if(r.action==='layout'){var layout=[];for(i=0;i<d.countPages();i++)layout.push(data(d,i,r.assetDir));return {pages:layout};}
 if(r.action==='render'){var sc=number(r.scale,1,0.1,4);checkedPixmap(d.loadPage(r.page),sc,false).saveAsPNG(r.output);return {};}
 if(r.action==='search'){var hits=[];for(i=0;i<d.countPages();i++){var q=d.loadPage(i).toStructuredText().search(r.query);if(q.length)hits.push({page:i,quads:q});}return hits;}
 if(r.action==='edit'){edit(d,r.edit);save(d,r.output);return info(d);}
 if(r.action==='convert'){if(d.layout)d.layout(595,842,12);var writer=new DocumentWriter(r.output,'pdf','compress');for(i=0;i<d.countPages();i++){p=d.loadPage(i);var dev=writer.beginPage(p.getBounds());p.run(dev,[1,0,0,1,0,0]);writer.endPage();}writer.close();return {};}
 selected=pages(o.pages,d.countPages(),r.action!=='delete');
 if(r.action==='merge'){var merged=new PDFDocument();for(i=0;i<r.inputs.length;i++){var src=open(r.inputs[i],o.password);for(var j=0;j<src.countPages();j++)merged.graftPage(-1,src,j);}d=merged;
 }else if(r.action==='extract'||r.action==='reorder'||r.action==='delete'){
  var seq=selected;if(r.action==='delete'){seq=[];for(i=0;i<d.countPages();i++)if(selected.indexOf(i)<0)seq.push(i);if(!seq.length)fail('Cannot delete every page');}d.rearrangePages(seq);
 }else if(r.action==='rotate'){var a=number(o.angle,90,-360,360);if(a%90)fail('Rotation must be a multiple of 90');for(i=0;i<selected.length;i++){var po=d.findPage(selected[i]);po.put('Rotate',(po.getInheritable('Rotate')?po.getInheritable('Rotate').asNumber():0)+a);}
 }else if(r.action==='crop'){for(i=0;i<selected.length;i++){p=d.loadPage(selected[i]);var b=p.getBounds(),l=number(o.left,20,0,10000),t=number(o.top,20,0,10000),rr=number(o.right,20,0,10000),bb=number(o.bottom,20,0,10000);p.setPageBox('CropBox',rect([b[0]+l,b[1]+t,b[2]-rr,b[3]-bb]));}
 }else if(r.action==='resize'){for(i=0;i<selected.length;i++){p=d.loadPage(selected[i]);var old=p.getBounds(),nw=number(o.width,595,36,14400),nh=number(o.height,842,36,14400),s=Math.min(nw/(old[2]-old[0]),nh/(old[3]-old[1])),obj=p.getObject(),c=obj.get('Contents'),ar=d.newArray(),m=Matrix.concat(p.getTransform(),[s,0,0,-s,(nw-(old[2]-old[0])*s)/2,(nh+(old[3]-old[1])*s)/2]);ar.push(d.addStream('q '+m.join(' ')+' cm'));if(c.isArray())for(var z=0;z<c.length;z++)ar.push(c.get(z));else ar.push(c);ar.push(d.addStream('Q'));obj.put('Contents',ar);obj.put('MediaBox',[0,0,nw,nh]);obj.put('CropBox',[0,0,nw,nh]);obj.put('Rotate',0);if(obj.get('Annots')&&obj.get('Annots').length)fail('Flatten annotations before resizing pages');}
 }else if(r.action==='watermark'){for(i=0;i<selected.length;i++){p=d.loadPage(selected[i]);var b=p.getBounds(),sz=number(o.size,32,4,200),box=[b[0]+30,(b[1]+b[3])/2-30,b[2]-30,b[3]-20];append(d,p,textContent(d,p,String(o.text||'BPDF'),box,sz,[.45,.45,.45],number(o.opacity,.25,0,1)));}
 }else if(r.action==='remove_watermark'){if(!o.watermarkText)fail('Enter the exact watermark text');var removed=0;for(i=0;i<selected.length;i++){p=d.loadPage(selected[i]);var ls=data(d,selected[i]).lines;for(var k=0;k<ls.length;k++)if(ls[k].text.trim()===String(o.watermarkText).trim()){removeText(p,ls[k].rect);removed++;}}if(!removed)fail('No independent matching text watermark found');
 }else if(r.action==='rasterize'||r.action==='grayscale'){d=raster(d,selected,o,r.action==='grayscale');
 }else if(r.action==='flatten'){d.bake(true,true);
 }else if(r.action==='protect'){if(!o.newPassword||String(o.newPassword).length<1)fail('Password required');if(/[\r\n,]/.test(o.newPassword))fail('Password must not contain commas or line breaks');save(d,r.output,'garbage=4,compress,encrypt=aes-256,user-password='+o.newPassword+',owner-password='+o.newPassword);return {};
 }else if(r.action==='unlock'||r.action==='repair'||r.action==='compress'){
 }else if(r.action==='to_images'){var outputs=[];var dpi=number(o.dpi,144,36,600);for(i=0;i<selected.length;i++){var path=r.output+'/page-'+('00000'+(selected[i]+1)).slice(-5)+'.png';checkedPixmap(d.loadPage(selected[i]),dpi/72,false).saveAsPNG(path);outputs.push(path);}return {outputs:outputs};
 }else fail('Unknown operation: '+r.action);
 save(d,r.output,r.action==='unlock'?'garbage=4,compress,encrypt=no':undefined);return {};
}
try { var request=JSON.parse(readFile(scriptArgs[0]).asString());var result=run(request);print('BPDF_RESULT:'+JSON.stringify({ok:true,result:result})); }
catch(e){print('BPDF_RESULT:'+JSON.stringify({ok:false,error:String(e.message||e)}));quit(1);}
