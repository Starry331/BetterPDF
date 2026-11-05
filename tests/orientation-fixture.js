var d=new PDFDocument(),font=d.addSimpleFont(new Font('Helvetica')),content='';
for(var i=0;i<38;i++)content+='BT /F 11 Tf 35 '+(800-i*19)+' Td (The local document contains readable words for automatic orientation.) Tj ET\n';
d.insertPage(-1,d.addPage([0,0,595,842],90,{Font:{F:font}},content));d.save(scriptArgs[0],'compress');
