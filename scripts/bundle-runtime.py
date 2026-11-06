#!/usr/bin/env python3
"""Assemble a relocatable offline macOS runtime. Does not install anything on end-user machines."""
import os,sys,shutil,subprocess
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
DEST=ROOT/'src-tauri/resources/runtime'
DEST.mkdir(parents=True,exist_ok=True)
BIN=DEST/'bin';LIB=DEST/'lib';NOTICE=DEST/'licenses'
for p in [BIN,LIB,NOTICE]:p.mkdir(exist_ok=True)
if sys.platform!='darwin':
 raise SystemExit('Use scripts/prepare-windows.ps1 for Windows runtime assembly.')
# Native workers always come from checksum-pinned source builds.
subprocess.run([sys.executable,str(ROOT/'scripts/build-native-macos.py'),'--install'],check=True)
source=Path(os.environ.get('BPDF_OFFICE_SOURCE','/Applications/LibreOffice.app'))
if not source.exists():raise SystemExit('Set BPDF_OFFICE_SOURCE to a relocatable headless LibreOffice application.')
if not (DEST/'office').exists():shutil.copytree(source,DEST/'office',symlinks=True)
# This source is already headless. Remove documentation/localizations, never conversion filters.
for pattern in ['Contents/Resources/help','Contents/Resources/readmes']:
 p=DEST/'office'/pattern
 if p.exists():shutil.rmtree(p)
gs=ROOT/'work/runtime-build/ghostpdl-10.08.0'
for name in ['Resource','lib','iccprofiles']:
 shutil.copytree(gs/name,DEST/'ghostscript'/name,dirs_exist_ok=True,symlinks=False)
tess=DEST/'tessdata';tess.mkdir(exist_ok=True)
for lang in ['eng','chi_sim','chi_tra','osd']:
 target=tess/(lang+'.traineddata')
 if not target.exists():
  print('Downloading bundled OCR model:',lang,flush=True)
  subprocess.run(['curl','-fL','--retry','3','--max-time','120','https://raw.githubusercontent.com/tesseract-ocr/tessdata_fast/master/'+lang+'.traineddata','-o',str(target)],check=True)
config=ROOT/'work/runtime-build/tesseract-5.5.4/tessdata/configs'
if not (tess/'configs').exists():shutil.copytree(config,tess/'configs')
subprocess.run([sys.executable,str(ROOT/'scripts/runtime-manifest.py')],check=True)
subprocess.run([sys.executable,str(ROOT/'scripts/audit-macos.py'),str(DEST),'--report',str(ROOT/'work/runtime-audit.json'),'--strict'],check=True)
