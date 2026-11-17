#!/usr/bin/env python3
"""Verify all manifest resources and ensure packaged symlinks stay inside the app."""
import argparse, hashlib, json
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('app',type=Path);p.add_argument('--report',type=Path,required=True);a=p.parse_args()
app=a.app.resolve();resources=app/'Contents/Resources/resources';runtime=resources/'runtime'
manifest=json.loads((runtime/'manifest.json').read_text());errors=[]
for item in manifest['files']:
    path=runtime/item['path']
    if not path.resolve().is_relative_to(runtime) or not path.is_file():errors.append('Missing/escaping: '+item['path']);continue
    h=hashlib.sha256()
    with path.open('rb') as stream:
        for block in iter(lambda:stream.read(1024*1024),b''):h.update(block)
    if path.stat().st_size!=item['bytes'] or h.hexdigest()!=item['sha256']:errors.append('Mismatch: '+item['path'])
for path in app.rglob('*'):
    if path.is_symlink() and (not path.exists() or not path.resolve().is_relative_to(app)):errors.append('External/broken symlink: '+str(path.relative_to(app)))
required=['bin/bpdf-render','bin/mutool','bin/qpdf','bin/gs','bin/tesseract','office/Contents/MacOS/soffice','ghostscript/Resource/Init/gs_init.ps']+[f'tessdata/{language}.traineddata' for language in ['eng','chi_sim','chi_tra','osd']]
for relative in required:
    if not (runtime/relative).is_file():errors.append('Required component missing: '+relative)
for relative in ['engine/worker.js']:
    if not (resources/relative).is_file():errors.append('Application engine missing: '+relative)
fonts=list((runtime/'office').rglob('*.ttf'))+list((runtime/'office').rglob('*.otf'))
if not fonts:errors.append('No bundled Office fonts')
report={'app':str(app),'passed':not errors,'verifiedRuntimeFiles':len(manifest['files']),'bundledFontFiles':len(fonts),'requiredComponents':required,'errors':errors,'scope':'Byte-level runtime manifest, required components and symlink containment; functional offline checks are separate'}
a.report.parent.mkdir(parents=True,exist_ok=True);a.report.write_text(json.dumps(report,indent=2));print(json.dumps(report,indent=2));raise SystemExit(bool(errors))
