#!/usr/bin/env python3
"""Assemble native Windows x64 workers and an offline, relocatable Office runtime.
Only used on the build machine; the installed app never runs this script.
"""
import argparse, hashlib, json, os, re, shutil, subprocess, sys, tarfile, urllib.request
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
RUNTIME=ROOT/'src-tauri/resources/runtime'
WORK=ROOT/'work/windows';WORK.mkdir(parents=True,exist_ok=True)
OFFICE_URL='https://download.documentfoundation.org/libreoffice/stable/26.8.3/win/x86_64/LibreOffice_26.8.3_Win_x86-64.msi'
OFFICE_SHA='0e6918c273eabbdbbd800ee0dacbbf130a14c13c2e4cec861b06dd59e015bf94'

def run(*args,**kwargs):
    return subprocess.run([str(x) for x in args],check=True,**kwargs)
def digest(p):
    with p.open('rb') as f:return hashlib.file_digest(f,'sha256').hexdigest()
def copy_tree(source,target):
    if not source.is_dir():raise RuntimeError(f'Missing component directory: {source}')
    shutil.copytree(source,target,dirs_exist_ok=True)
def main():
    if sys.platform!='win32':raise SystemExit('Run on Windows x64 with MSYS2 UCRT64 tools.')
    parser=argparse.ArgumentParser();parser.add_argument('--msys',type=Path,required=True);a=parser.parse_args()
    prefix=a.msys/'ucrt64';bin=prefix/'bin';dest=RUNTIME/'bin';dest.mkdir(parents=True,exist_ok=True)
    os.environ['PATH']=str(bin)+os.pathsep+os.environ['PATH']
    # Resolve only actual PE imports. No build tools or MSYS POSIX runtime ship.
    available={p.name.lower():p for p in bin.glob('*') if p.is_file()}
    copied=set()
    def copy_binary(source):
        key=source.name.lower()
        if key in copied:return
        copied.add(key);shutil.copy2(source,dest/source.name)
        output=subprocess.check_output([str(bin/'objdump.exe'),'-p',str(source)],text=True,errors='replace')
        if 'pei-x86-64' not in output:raise RuntimeError(f'Not an x64 PE: {source}')
        for name in re.findall(r'DLL Name:\s*(\S+)',output):
            dependency=available.get(name.lower())
            if dependency:copy_binary(dependency)
            elif name.lower().startswith(('api-ms-win-','ext-ms-win-')):continue
            elif not (Path(os.environ['SystemRoot'])/'System32'/name).is_file():raise RuntimeError(f'Unresolved DLL: {name} in {source}')
    for name in ['mutool.exe','qpdf.exe','gs.exe','tesseract.exe']:copy_binary(bin/name)
    run(bin/'gcc.exe','-O2','-s','-municode','-I'+str(prefix/'include'),ROOT/'native/reader.c','-L'+str(prefix/'lib'),'-lmupdf','-o',WORK/'bpdf-render.exe')
    copy_binary(WORK/'bpdf-render.exe')
    run(bin/'gcc.exe','-O2','-s','-municode','-I'+str(prefix/'include'),ROOT/'native/print-windows.c','-L'+str(prefix/'lib'),'-lmupdf','-lcomdlg32','-lgdi32','-o',WORK/'bpdf-print.exe')
    copy_binary(WORK/'bpdf-print.exe')
    gs_versions=list((prefix/'share/ghostscript').glob('*'))
    gs=next(p for p in gs_versions if (p/'lib').is_dir())
    for name in ['Resource','lib','iccprofiles']:
        source=gs/name
        if source.is_dir():copy_tree(source,RUNTIME/'ghostscript'/name)
    # MSYS2 embeds PostScript resources in its executable; bundle pinned profiles
    # explicitly so PDF/A does not depend on the build-machine installation.
    spec=next(x for x in json.loads((ROOT/'scripts/native-sources.json').read_text()) if x['directory'].startswith('ghostpdl-'))
    archive=WORK/spec['archive']
    if not archive.exists():
        with urllib.request.urlopen(spec['url'],timeout=120) as response,archive.open('wb') as f:shutil.copyfileobj(response,f)
    if digest(archive)!=spec['sha256']:raise RuntimeError('Ghostscript source checksum mismatch')
    with tarfile.open(archive) as tar:
        for member in tar:
            parts=Path(member.name).parts
            if member.isfile() and len(parts)>2 and parts[0]==spec['directory'] and parts[1] in ('Resource','iccprofiles'):
                target=RUNTIME/'ghostscript'/Path(*parts[1:])
                if not target.resolve().is_relative_to((RUNTIME/'ghostscript').resolve()):raise RuntimeError('Unsafe archive path')
                target.parent.mkdir(parents=True,exist_ok=True)
                with tar.extractfile(member) as source,target.open('wb') as output:shutil.copyfileobj(source,output)
    if not (RUNTIME/'ghostscript/iccprofiles/srgb.icc').is_file():raise RuntimeError('Missing PDF/A color profile')
    copy_tree(prefix/'share/tessdata/configs',RUNTIME/'tessdata/configs')
    run(sys.executable,ROOT/'scripts/prepare_ocr_models.py')
    copy_tree(prefix/'share/licenses',RUNTIME/'licenses/msys2')
    packages=subprocess.check_output([str(a.msys/'usr/bin/bash.exe'),'-lc','pacman -Q'],text=True)
    (RUNTIME/'licenses/msys2-packages.txt').write_text(packages,encoding='utf-8')
    # Keep build provenance; pacman verifies repository package signatures.
    receipts=[]
    for package in (a.msys/'var/cache/pacman/pkg').glob('*.pkg.tar.*'):
        if package.suffix=='.sig':continue
        receipts.append({'archive':package.name,'sha256':digest(package)})
    (RUNTIME/'licenses/windows-provenance.json').write_text(json.dumps({'office':{'url':OFFICE_URL,'sha256':OFFICE_SHA},'msys2Packages':receipts,'recipes':'https://github.com/msys2/MINGW-packages'},indent=2))
    msi=WORK/'LibreOffice.msi'
    if not msi.exists():
        with urllib.request.urlopen(OFFICE_URL,timeout=120) as response,msi.open('wb') as f:shutil.copyfileobj(response,f)
    if digest(msi)!=OFFICE_SHA:raise RuntimeError('LibreOffice MSI checksum mismatch')
    office=RUNTIME/'office'
    if not (office/'program/soffice.exe').is_file():
        extract=WORK/'office-extracted';extract.mkdir(exist_ok=True)
        run('msiexec.exe','/a',msi,'/qn',f'TARGETDIR={extract.resolve()}','/L*v',WORK/'office-extract.log')
        program=next(extract.rglob('soffice.exe')).parent
        copy_tree(program.parent,office)
    # Remove only optional help and dictionaries; retain conversion filters.
    for name in ['help','readmes']:
        shutil.rmtree(office/name,ignore_errors=True)
    for path in (office/'share/extensions').glob('dict-*'):
        if not path.name.startswith(('dict-en','dict-zh')):shutil.rmtree(path)
    # Fonts extracted by the MSI outside the Office directory are still bundled.
    fonts=office/'share/fonts/truetype';fonts.mkdir(parents=True,exist_ok=True)
    for p in (WORK/'office-extracted').rglob('*'):
        if p.suffix.lower() in ('.ttf','.otf','.ttc') and not (fonts/p.name).exists():shutil.copy2(p,fonts/p.name)
    run(sys.executable,ROOT/'scripts/runtime-manifest.py')
    print(f'Windows runtime: {sum(p.stat().st_size for p in RUNTIME.rglob("*") if p.is_file())/1e6:.2f} MB')
if __name__=='__main__':main()
