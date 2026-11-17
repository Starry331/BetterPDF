#!/usr/bin/env python3
"""Package the latest local app without claiming full-release acceptance or signing."""
import argparse, hashlib, json, os, shutil, subprocess, tempfile
from pathlib import Path
root=Path(__file__).resolve().parents[1]
app=root/'src-tauri/target/release/bundle/macos/BPDF.app'
parser=argparse.ArgumentParser();parser.add_argument('--output-dir',type=Path,default=root/'outputs');args=parser.parse_args()
out=args.output_dir.resolve()
out.mkdir(parents=True,exist_ok=True)
subprocess.run(['python3',str(root/'scripts/verify-bundle.py'),str(app),'--report',str(root/'work/bundle-verification.json')],check=True)
with tempfile.TemporaryDirectory(prefix='dmg-',dir=root/'work') as temp:
    temp=Path(temp);stage=temp/'stage';stage.mkdir();shutil.copytree(app,stage/'BPDF.app',symlinks=True)
    (stage/'Applications').symlink_to('/Applications')
    (stage/'README.txt').write_text('Better PDF — local development build\n\nDrag BPDF.app to Applications. macOS 14+, Apple Silicon.\nAll implemented tools, conversion engines, OCR models and fonts are bundled.\nNo account, activation or additional component download is required.\nThis download is not Developer ID signed/notarized.\nThis is a development build, not a completed first release.\nSee bundled STATUS.md for known limitations and verification scope.\n',encoding='utf-8')
    shutil.copyfile(root/'docs/STATUS.md',stage/'STATUS.md')
    if (root/'docs/PRESSURE_TESTS.md').exists():shutil.copyfile(root/'docs/PRESSURE_TESTS.md',stage/'PRESSURE_TESTS.md')
    target=temp/'BetterPDF-latest-macOS-arm64.dmg'
    subprocess.run(['hdiutil','create','-volname','Better PDF','-srcfolder',str(stage),'-format','UDZO',str(target)],check=True)
    subprocess.run(['hdiutil','verify',str(target)],check=True)
    destination=out/target.name;os.replace(target,destination)
    h=hashlib.sha256()
    with destination.open('rb') as stream:
        for block in iter(lambda:stream.read(1024*1024),b''):h.update(block)
    (out/'SHA256SUMS-latest.txt').write_text(f'{h.hexdigest()}  {destination.name}\n')
    print(json.dumps({'dmg':str(destination),'bytes':destination.stat().st_size,'sha256':h.hexdigest()}))
