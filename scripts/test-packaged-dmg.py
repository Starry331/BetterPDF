#!/usr/bin/env python3
"""Mount the delivered DMG read-only; run all tools with its bundled runtime.
The external test harness exercises the same Rust pipeline; document workers deny network.
"""
import argparse, json, os, shutil, subprocess, tempfile
from pathlib import Path
root=Path(__file__).resolve().parents[1];work=root/'work'
parser=argparse.ArgumentParser();parser.add_argument('--dmg',type=Path,default=root/'outputs/BetterPDF-latest-macOS-arm64.dmg');args=parser.parse_args();dmg=args.dmg.resolve()
node=shutil.which('node');assert node
with tempfile.TemporaryDirectory(prefix='mounted-dmg-',dir=work) as folder:
    mount=Path(folder)/'mount';mount.mkdir();attached=False
    try:
        subprocess.run(['hdiutil','attach','-readonly','-nobrowse','-mountpoint',str(mount),str(dmg)],check=True,capture_output=True);attached=True
        app=mount/'BPDF.app'
        subprocess.run(['python3',str(root/'scripts/verify-bundle.py'),str(app),'--report',str(work/'dmg-bundle-verification.json')],check=True,capture_output=True)
        env=dict(os.environ,BPDF_RESOURCES=str(app/'Contents/Resources/resources'),BPDF_TEST_CLI=str(root/'src-tauri/target/release/bpdf-cli'),PATH='/usr/bin:/bin:/usr/sbin:/sbin')
        checks=[]
        for suite in ['engine','office']:
            with (work/f'dmg-{suite}-tests.log').open('w') as log:
                run=subprocess.run([node,str(root/f'tests/{suite}.integration.mjs')],cwd=root,env=env,stdout=log,stderr=subprocess.STDOUT,timeout=900)
            checks.append({'suite':suite,'passed':run.returncode==0,'log':f'work/dmg-{suite}-tests.log'})
        report={'dmg':str(dmg),'scope':'All tool and Office integration fixtures use the read-only DMG runtime; minimal PATH; workers deny network. Not a clean macOS 14 installation or GUI soak test.','checks':checks}
        (work/'dmg-functional-results.json').write_text(json.dumps(report,indent=2));print(json.dumps(report,indent=2))
        assert all(c['passed'] for c in checks),'Packaged integration failed; see logs'
    finally:
        if attached:subprocess.run(['hdiutil','detach',str(mount)],check=True,capture_output=True)
