#!/usr/bin/env python3
"""Build a clearly non-release application with the real runtime deployment target."""
import json,subprocess,sys
from pathlib import Path
root=Path(__file__).resolve().parents[1];work=root/'work';work.mkdir(exist_ok=True)
if sys.platform!='darwin':raise SystemExit('Development runtime is currently assembled for macOS only.')
report=work/'runtime-audit.json'
subprocess.run([sys.executable,str(root/'scripts/audit-macos.py'),str(root/'src-tauri/resources/runtime'),'--report',str(report)],check=True)
audit=json.loads(report.read_text());versions=[b['minimumOS'] for b in audit['binaries']]
minimum=max(['14.0']+versions,key=lambda v:tuple((list(map(int,v.split('.')))+[0,0])[:3]))
config=work/'dev-build.json';config.write_text(json.dumps({'bundle':{'macOS':{'minimumSystemVersion':minimum}}}))
print('Building local development app; minimum OS:',minimum,flush=True)
subprocess.run(['npm','run','tauri','--','build','--bundles','app','--config',str(config)],cwd=root,check=True)
