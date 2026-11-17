#!/usr/bin/env python3
"""Verify bundled OCR models and atomically fetch missing pinned artifacts at build time."""
import hashlib
import json
import os
import shutil
from pathlib import Path
import tempfile
import urllib.request

ROOT = Path(__file__).resolve().parents[1]

def sha256(path):
    digest = hashlib.sha256()
    with path.open('rb') as source:
        for chunk in iter(lambda: source.read(65536), b''):
            digest.update(chunk)
    return digest.hexdigest()

def ensure_model(destination, spec):
    name = spec['language']
    if name not in ('eng', 'chi_sim', 'chi_tra', 'osd'):
        raise ValueError('Unsupported bundled OCR language')
    destination.mkdir(parents=True, exist_ok=True)
    target = destination / (name + '.traineddata')
    if target.exists():
        if target.stat().st_size != spec['bytes'] or sha256(target) != spec['sha256']:
            raise ValueError('Bundled OCR model failed integrity verification: ' + name)
        return
    temporary = None
    try:
        with tempfile.NamedTemporaryFile(dir=destination, prefix='ocr-', delete=False) as output:
            temporary = Path(output.name)
            size = 0
            digest = hashlib.sha256()
            with urllib.request.urlopen(spec['url'], timeout=120) as response:
                while True:
                    chunk = response.read(65536)
                    if not chunk:
                        break
                    size += len(chunk)
                    if size > spec['bytes']:
                        raise ValueError('OCR download exceeds pinned size: ' + name)
                    digest.update(chunk)
                    output.write(chunk)
            if size != spec['bytes'] or digest.hexdigest() != spec['sha256']:
                raise ValueError('OCR download failed integrity verification: ' + name)
            output.flush()
            os.fsync(output.fileno())
        os.replace(temporary, target)
    finally:
        if temporary is not None:
            temporary.unlink(missing_ok=True)

if __name__ == '__main__':
    manifest = json.loads((ROOT / 'scripts/ocr-models.json').read_text())
    for spec in manifest['models']:
        ensure_model(ROOT / 'src-tauri/resources/runtime/tessdata', spec)
    license_source = ROOT / manifest['license']['path']
    if sha256(license_source) != manifest['license']['sha256']:
        raise ValueError('Pinned OCR model license failed verification')
    notices = ROOT / 'src-tauri/resources/runtime/licenses/tessdata-fast'
    notices.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(license_source, notices / 'LICENSE')
    shutil.copyfile(ROOT / 'scripts/ocr-models.json', notices / 'sources.json')
    print('All four bundled OCR models and their license match pinned sources.')
