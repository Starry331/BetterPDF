#!/usr/bin/env python3
"""Install the built package and test only installed resources, with no build tools on PATH."""
import hashlib,json,os,subprocess,time
from pathlib import Path
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
sequence=0;results=[]
def request(name,req,expected=True):
 global sequence
 sequence+=1;p=work/f'request-{sequence}.json';p.write_text(json.dumps(req),encoding='utf-8');start=time.monotonic()
 result=subprocess.run([str(cli),str(p)],env=env,capture_output=True,text=True,encoding='utf-8',timeout=240)
 parsed=json.loads(result.stdout);assert parsed['ok']==expected,(name,parsed,result.stderr)
 results.append({'name':name,'seconds':round(time.monotonic()-start,3),'ok':True});return parsed
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
report={'passed':True,'runtimeFilesVerified':len(manifest['files']),'installedBytes':sum(p.stat().st_size for p in installed.rglob('*') if p.is_file()),'installerBytes':installer.stat().st_size,'tests':results,'scope':'Windows Server 2022 runner; installed resource and basic 1000-page/cancellation checks. Not Windows 11 GUI or clean-machine certification. WebView2 system footprint excluded and must be reported separately.'}
(work/'installed-validation.json').write_text(json.dumps(report,indent=2),encoding='utf-8');print(json.dumps(report,indent=2))
