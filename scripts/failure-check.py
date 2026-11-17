#!/usr/bin/env python3
"""Exercise a real full temporary filesystem; never fill the user's system disk."""
import errno
import hashlib
import json
import platform
import subprocess
import tempfile
from pathlib import Path

root = Path(__file__).resolve().parents[1]
work = root / 'work'
cli = root / 'src-tauri/target/debug/bpdf-cli'
source = Path(json.loads((work / 'integration-results.json').read_text())['run']) / 'sample.pdf'
if platform.system() != 'Darwin':
    raise SystemExit('This isolated disk-full check requires macOS.')
digest = hashlib.sha256(source.read_bytes()).hexdigest()
results = []
with tempfile.TemporaryDirectory(prefix='bpdf-failure-', dir=work) as folder:
    folder = Path(folder)
    image = folder / 'full.sparseimage'
    mount = folder / 'mount'
    mount.mkdir()
    subprocess.run(['hdiutil', 'create', '-size', '32m', '-type', 'SPARSE', '-fs', 'HFS+', '-volname', 'BPDF Failure Test', str(image)], check=True, capture_output=True)
    attached = False
    try:
        subprocess.run(['hdiutil', 'attach', '-nobrowse', '-mountpoint', str(mount), str(image)], check=True, capture_output=True)
        attached = True
        with (mount / 'filler').open('wb', buffering=0) as filler:
            try:
                for _ in range(1024):
                    filler.write(b'x' * 65536)
            except OSError as error:
                if error.errno != errno.ENOSPC:
                    raise
            else:
                raise AssertionError('Expected the isolated 32 MB volume to fill')
        # Consume any remaining allocation blocks after the first large write failed.
        with (mount / 'filler').open('ab', buffering=0) as filler:
            try:
                for _ in range(65536):
                    filler.write(b'x')
            except OSError as error:
                if error.errno != errno.ENOSPC:
                    raise
        destination = mount / 'result.pdf'
        request = folder / 'request.json'
        request.write_text(json.dumps({'mode':'job', 'tool':'rotate', 'inputs':[str(source)], 'output':str(destination), 'options':{'angle':90}}))
        response = subprocess.run([cli, request], capture_output=True, text=True, timeout=60)
        value = json.loads(response.stdout)
        assert not value['ok'], value
        assert not destination.exists(), 'A partial output was published'
        assert hashlib.sha256(source.read_bytes()).hexdigest() == digest
        results.append({'name':'full-volume rejects conversion without publishing output or changing source', 'passed':True, 'error':value.get('error')})
        request.write_text(json.dumps({'action':'info', 'input':str(source)}))
        value = json.loads(subprocess.run([cli, request], capture_output=True, text=True, timeout=60).stdout)
        assert value['ok'] and value['result']['pages'] > 0
        results.append({'name':'native reader remains usable after write failure', 'passed':True})
    finally:
        if attached:
            subprocess.run(['hdiutil', 'detach', str(mount)], check=True, capture_output=True)
report = {'platform':platform.platform(), 'scope':'Real ENOSPC on a private 32 MB mounted image; conversion path, not GUI save-dialog acceptance', 'results':results}
(work / 'failure-results.json').write_text(json.dumps(report, indent=2))
print(json.dumps(report, indent=2))
