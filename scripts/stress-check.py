#!/usr/bin/env python3
"""Synthetic 1000-page / ~28 MB smoke benchmark, not a baseline-device acceptance test."""
from pathlib import Path
import json,random,subprocess,time,re,platform
root=Path(__file__).resolve().parents[1];work=root/'work/stress';work.mkdir(parents=True,exist_ok=True)
image=work/'noise.ppm';rng=random.Random(2026)
with image.open('wb') as f:
 f.write(b'P6\n3072 3072\n255\n')
 for _ in range(3072):f.write(rng.randbytes(3072*3))
runtime=root/'src-tauri/resources/runtime';cli=root/'src-tauri/target/debug/bpdf-cli';pdf=work/'1000-pages.pdf';huge=work/'oversized.pdf'
subprocess.run([runtime/'bin/mutool','run',root/'tests/stress-fixture.js',pdf,image,huge],check=True)
results=[]
def run(name,request,expected=True):
 req=work/(name+'.json');req.write_text(json.dumps(request));cmd=[str(cli),str(req)]
 if platform.system()=='Darwin':cmd=['/usr/bin/time','-l']+cmd
 start=time.monotonic();p=subprocess.run(cmd,capture_output=True,text=True,timeout=120);value=json.loads(p.stdout);rss=re.search(r'(\d+)\s+maximum resident set size',p.stderr)
 results.append({'name':name,'seconds':round(time.monotonic()-start,3),'peakProcessBytes':int(rss[1]) if rss else None,'expectedResult':value['ok']==expected,'error':value.get('error')})
 assert value['ok']==expected,value
 return value
run('open',{'action':'info','input':str(pdf)})
for page in [0,499,999]:run('render-'+str(page),{'action':'render','input':str(pdf),'page':page,'scale':1,'output':str(work/f'page-{page}.png')})
run('reject-oversized-page',{'action':'render','input':str(huge),'page':0,'scale':1,'output':str(work/'too-big.png')},False)
run('read-after-rejection',{'action':'page','input':str(pdf),'page':999})
report={'platform':platform.platform(),'pdfBytes':pdf.stat().st_size,'pageCount':1000,'scope':'Native process smoke test only; does not measure total GUI memory or target baseline hardware','results':results}
(work/'results.json').write_text(json.dumps(report,indent=2));print(json.dumps(report,indent=2))
