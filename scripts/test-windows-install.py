#!/usr/bin/env python3
"""Install the built package and test only installed resources, with no build tools on PATH."""
import hashlib,json,os,shutil,subprocess,sys,time
from pathlib import Path
import psutil
root=Path(__file__).resolve().parents[1];work=root/'work/windows';work.mkdir(parents=True,exist_ok=True)
installer=next((root/'src-tauri/target/release/bundle/nsis').glob('*-setup.exe'))
installed=Path(os.environ['LOCALAPPDATA'])/'BetterPDF 验证'
subprocess.run([str(installer),'/S',f'/D={installed}'],check=True,timeout=300)
resources=installed/'resources';runtime=resources/'runtime'
manifest=json.loads((runtime/'manifest.json').read_text());errors=[]
for item in manifest['files']:
 p=runtime/item['path']
 if not p.resolve().is_relative_to(runtime.resolve()) or not p.is_file():errors.append(item['path']);continue
 with p.open('rb') as f:h=hashlib.file_digest(f,'sha256').hexdigest()
 if p.stat().st_size!=item['bytes'] or h!=item['sha256']:errors.append(item['path'])
assert not errors,errors
cli=root/'src-tauri/target/debug/bpdf-cli.exe'
env=dict(os.environ,BPDF_RESOURCES=str(resources),PATH=str(Path(os.environ['SystemRoot'])/'System32'))
# Exercise installed engines rather than whatever happens to be on the runner.
node=shutil.which('node');assert node
for suite in ['engine.integration.mjs','office.integration.mjs']:
 subprocess.run([node,str(root/'tests'/suite)],env=env,cwd=root,check=True,timeout=1800)
subprocess.run([node,str(root/'tests/office.integration.mjs')],env=dict(env,BPDF_TEST_LEGACY='1'),cwd=root,check=True,timeout=600)
sequence=0;results=[]
def request(name,req,expected=True):
 global sequence
 sequence+=1;p=work/f'request-{sequence}.json';p.write_text(json.dumps(req),encoding='utf-8');start=time.monotonic()
 peak=0;out=work/f'output-{sequence}.json';err=work/f'error-{sequence}.log'
 with out.open('wb') as stdout,err.open('wb') as stderr:
  process=subprocess.Popen([str(cli),str(p)],env=env,stdout=stdout,stderr=stderr)
  try:
   while process.poll() is None:
    try:
     parent=psutil.Process(process.pid);tree=[parent]+parent.children(recursive=True)
     rss=sum(child.memory_info().rss for child in tree if child.is_running());peak=max(peak,rss)
    except psutil.Error:pass
    if time.monotonic()-start>240 or peak>2*1024**3:raise RuntimeError(f'Pressure budget exceeded: {name}')
    time.sleep(.05)
  finally:
   if process.poll() is None:
    process.kill();process.wait()  # Job handles close and terminate its worker tree.
 parsed=json.loads(out.read_text(encoding='utf-8'));assert parsed['ok']==expected,(name,parsed,err.read_text(errors='replace'))
 results.append({'name':name,'seconds':round(time.monotonic()-start,3),'peakProcessTreeBytes':peak,'ok':True});return parsed
source=work/'中文 测试.pdf'
request('1000-page fixture',{'action':'fixture','output':str(source),'pages':1000})
original=hashlib.sha256(source.read_bytes()).hexdigest()
for page in [0,499,999]*3:
 request(f'render {page}',{'action':'render','input':str(source),'output':str(work/'页面.png'),'page':page,'scale':1})
for n in range(3):
 output=work/f'cancel-{n}.pdf'
 result=request('cancel rasterization',{'mode':'job','tool':'rasterize','inputs':[str(source)],'output':str(output),'options':{'dpi':150},'cancelAfterMs':100},False)
 assert result['error']=='CANCELLED' and not output.exists(),result
request('reader recovers after cancellation',{'action':'info','input':str(source)})
assert hashlib.sha256(source.read_bytes()).hexdigest()==original
subprocess.run([sys.executable,str(root/'scripts/large-stress-fixture.py')],env=env,check=True,timeout=240)
large=root/'work/stress/1000-pages-200MB.pdf'
request('200 MB / 1000 pages rasterization',{'mode':'job','tool':'rasterize','inputs':[str(large)],'output':str(work/'large-raster.pdf'),'options':{'dpi':36}})
request('reader after large task',{'action':'info','input':str(large)})
report={'passed':True,'runtimeFilesVerified':len(manifest['files']),'installedBytes':sum(p.stat().st_size for p in installed.rglob('*') if p.is_file()),'installerBytes':installer.stat().st_size,'tests':results,'scope':'Windows Server 2022 runner; installed resource and basic 1000-page/cancellation checks. Not Windows 11 GUI or clean-machine certification. WebView2 system footprint excluded and must be reported separately.'}
(work/'installed-validation.json').write_text(json.dumps(report,indent=2),encoding='utf-8');print(json.dumps(report,indent=2))
