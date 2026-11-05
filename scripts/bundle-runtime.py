#!/usr/bin/env python3
"""Assemble a relocatable offline macOS runtime. Does not install anything on end-user machines."""
import os,sys,shutil,subprocess,json,hashlib,urllib.request
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
DEST=ROOT/'src-tauri/resources/runtime'
DEST.mkdir(parents=True,exist_ok=True)
BIN=DEST/'bin';LIB=DEST/'lib';NOTICE=DEST/'licenses'
for p in [BIN,LIB,NOTICE]:p.mkdir(exist_ok=True)
if sys.platform!='darwin':
 raise SystemExit('Use scripts/prepare-windows.ps1 for Windows runtime assembly.')
seen={}
def run(*args):return subprocess.check_output(args,text=True,stderr=subprocess.STDOUT).strip()
def bundle(source,target):
 source=Path(source).resolve();target=Path(target)
 if str(source) in seen:return seen[str(source)]
 seen[str(source)]=target
 shutil.copy2(source,target);os.chmod(target,0o755)
 subprocess.run(['codesign','--remove-signature',str(target)],capture_output=True)
 deps=[]
 for line in run('otool','-L',str(source)).splitlines()[1:]:
  dep=line.strip().split(' (')[0]
  resolved=None
  if dep.startswith('/opt/homebrew/'):resolved=Path(dep)
  elif dep.startswith('@rpath/'):
   candidates=[source.parent/Path(dep).name,source.parent.parent/'lib'/Path(dep).name,Path('/opt/homebrew/lib')/Path(dep).name]
   resolved=next((p for p in candidates if p.exists()),None)
   if resolved is None:raise RuntimeError(f'Unresolved library {dep} from {source}')
  elif dep.startswith('@loader_path/'):
   resolved=source.parent/dep.removeprefix('@loader_path/')
  if resolved and resolved.resolve()!=source:
   copied=bundle(resolved,LIB/Path(dep).name)
   rel='@loader_path/'+os.path.relpath(copied,target.parent)
   deps.extend(['-change',dep,rel])
 if deps:subprocess.run(['install_name_tool',*deps,str(target)],check=True)
 if target.suffix=='.dylib':subprocess.run(['install_name_tool','-id','@rpath/'+target.name,str(target)],check=True)
 subprocess.run(['codesign','--force','--sign','-',str(target)],check=True,capture_output=True)
 # Preserve package license material for every copied Homebrew dependency.
 parts=source.parts
 if 'Cellar' in parts:
  k=parts.index('Cellar');prefix=Path(*parts[:k+3]);name=parts[k+1];nd=NOTICE/name;nd.mkdir(exist_ok=True)
  for pat in ['LICENSE*','COPYING*','NOTICE*','AUTHORS*']:
   for f in prefix.glob(pat):
    if f.is_file():shutil.copy2(f,nd/f.name)
  for d in [prefix/'share/doc'/name]:
   if d.exists():
    for pat in ['LICENSE*','COPYING*','NOTICE*']:
     for f in d.glob(pat):
      if f.is_file():shutil.copy2(f,nd/f.name)
 return target
for name in ['mutool','qpdf','gs','tesseract']:
 src=shutil.which(name)
 if not src:raise SystemExit(f'Missing build dependency: {name}')
 bundle(src,BIN/name)
source=Path(os.environ.get('BPDF_OFFICE_SOURCE','/Applications/LibreOffice.app'))
if not source.exists():raise SystemExit('Set BPDF_OFFICE_SOURCE to a relocatable headless LibreOffice application.')
if not (DEST/'office').exists():shutil.copytree(source,DEST/'office',symlinks=True)
# This source is already headless. Remove documentation/localizations, never conversion filters.
for pattern in ['Contents/Resources/help','Contents/Resources/readmes']:
 p=DEST/'office'/pattern
 if p.exists():shutil.rmtree(p)
gs=Path(run('brew','--prefix','ghostscript'))/'share/ghostscript'
if not (DEST/'ghostscript').exists():shutil.copytree(gs,DEST/'ghostscript',symlinks=False)
tess=DEST/'tessdata';tess.mkdir(exist_ok=True)
for lang in ['eng','chi_sim','chi_tra','osd']:
 target=tess/(lang+'.traineddata')
 if not target.exists():
  print('Downloading bundled OCR model:',lang,flush=True)
  subprocess.run(['curl','-fL','--retry','3','--max-time','120','https://raw.githubusercontent.com/tesseract-ocr/tessdata_fast/master/'+lang+'.traineddata','-o',str(target)],check=True)
config=Path(run('brew','--prefix','tesseract'))/'share/tessdata/configs'
if not (tess/'configs').exists():shutil.copytree(config,tess/'configs')
# Build the thin C rendering process against the same native engine.
include=Path(run('brew','--prefix','mupdf-tools'))
with (ROOT/'work/native-build.log').open('w') as log:
 subprocess.run(['cc','-O2','-I'+str(include/'include'),str(ROOT/'native/reader.c'),'-L'+str(include/'lib'),'-lmupdf','-lmupdf-third','-lm','-lpthread','-o',str(BIN/'bpdf-render')],stdout=log,stderr=log,check=True)
subprocess.run(['codesign','--force','--sign','-',str(BIN/'bpdf-render')],check=True,capture_output=True)
files=[]
for p in sorted(DEST.rglob('*')):
 if p.is_file() and not p.is_symlink():files.append({'path':str(p.relative_to(DEST)),'bytes':p.stat().st_size,'sha256':hashlib.sha256(p.read_bytes()).hexdigest()})
manifest={'platform':'macos-arm64','files':files,'totalBytes':sum(x['bytes'] for x in files),'sourceOffice':str(source),'components':{n:run(str(BIN/n),*({'mutool':['-v'],'qpdf':['--version'],'gs':['--version'],'tesseract':['--version']}[n])).splitlines()[0] for n in ['mutool','qpdf','gs','tesseract']}}
(DEST/'manifest.json').write_text(json.dumps(manifest,ensure_ascii=False,indent=2))
print(f'Runtime: {manifest["totalBytes"]/1_000_000:.1f} MB')
