var d=new PDFDocument(),font=d.addSimpleFont(new Font('Helvetica')),img=d.addImage(new Image(scriptArgs[1]));
for(var i=0;i<1000;i++)d.insertPage(-1,d.addPage([0,0,595,842],0,{Font:{F:font},XObject:{Im:img}},'BT /F 18 Tf 40 800 Td (Large document page '+(i+1)+') Tj ET q 500 0 0 500 40 200 cm /Im Do Q'));
d.save(scriptArgs[0],'compress');
var huge=new PDFDocument();huge.insertPage(-1,huge.addPage([0,0,100000,100000],0,{},''));huge.save(scriptArgs[2]);
