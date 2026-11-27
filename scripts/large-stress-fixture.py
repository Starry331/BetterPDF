#!/usr/bin/env python3
"""Create a 1000-page ~200 MB fixture with 64 independent noise images."""
import json, os, random, subprocess, tempfile
from pathlib import Path
root=Path(__file__).resolve().parents[1];work=root/'work/stress';work.mkdir(parents=True,exist_ok=True)
rng=random.Random(20261117)
with tempfile.TemporaryDirectory(dir=work,prefix='image-fixtures-') as folder:
    paths=[]
    for i in range(64):
        path=Path(folder)/f'{i}.ppm';paths.append(str(path))
        with path.open('wb') as f:
            f.write(b'P6\n1024 1024\n255\n')
            for _ in range(1024):f.write(rng.randbytes(3072))
    output=work/'1000-pages-200MB.pdf'
    script=Path(folder)/'fixture.js'
    script.write_text('var d=new PDFDocument(), images='+json.dumps(paths)+'; for(var i=0;i<images.length;i++)images[i]=d.addImage(new Image(images[i]));for(var p=0;p<1000;p++)d.insertPage(-1,d.addPage([0,0,595,842],0,{XObject:{Im:images[p%64]}},"q 500 0 0 500 40 200 cm /Im Do Q")); d.save('+json.dumps(str(output))+',"compress");')
    subprocess.run([Path(os.environ.get('BPDF_RESOURCES',root/'src-tauri/resources'))/'runtime/bin'/('mutool.exe' if os.name=='nt' else 'mutool'),'run',script],check=True,timeout=180)
    print(json.dumps({'path':str(output),'bytes':output.stat().st_size,'pages':1000,'independentImages':64}))
