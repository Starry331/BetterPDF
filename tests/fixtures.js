var d = new PDFDocument();
var f = d.addSimpleFont(new Font('Helvetica'));
var content = 'BT /F 24 Tf 50 770 Td (BPDF Integration Test) Tj ET\n';
content += 'BT /F 12 Tf 50 735 Td (Original editable sentence.) Tj ET\n';
var rows = [['Item','Quantity','Price'],['Notebook','3','12.50'],['Pencil','10','2.00'],['Total','13','57.50']];
for(var i=0;i<rows.length;i++)for(var j=0;j<3;j++)content+='BT /F 12 Tf '+(50+j*160)+' '+(650-i*25)+' Td ('+rows[i][j]+') Tj ET\n';
for(var p=0;p<3;p++)d.insertPage(-1,d.addPage([0,0,595,842],0,{Font:{F:f}},content+'BT /F 10 Tf 50 30 Td (Page '+(p+1)+') Tj ET'));
var root=d.getTrailer().get('Root');var widget=d.addObject({Type:'Annot',Subtype:'Widget',FT:'Tx',T:d.newString('full_name'),V:d.newString('Alice'),Rect:[50,450,300,475],DA:d.newString('/F 12 Tf 0 g'),F:4});
var page=d.findPage(0);page.put('Annots',[widget]);root.put('AcroForm',{Fields:[widget],DR:{Font:{F:f}},DA:d.newString('/F 12 Tf 0 g')});d.loadPage(0).update();d.save(scriptArgs[0],'compress,garbage=4');
