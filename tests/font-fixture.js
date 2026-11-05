var d=new PDFDocument(),f=d.addSimpleFont(new Font('Times-BoldItalic'));
d.insertPage(-1,d.addPage([0,0,595,842],0,{Font:{F:f}},'0.2 0.4 0.6 rg BT /F 18 Tf 50 700 Td (Original styled text) Tj ET'));
d.save(scriptArgs[0],'compress');
