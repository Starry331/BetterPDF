#!/usr/bin/env python3
"""Record the installed runtime contents, excluding the manifest itself."""
import hashlib
import json
from pathlib import Path
import subprocess

root = Path(__file__).resolve().parents[1]
runtime = root / 'src-tauri/resources/runtime'
files = []
for path in sorted(runtime.rglob('*')):
    if not path.is_file() or path.is_symlink() or path == runtime / 'manifest.json':
        continue
    digest = hashlib.sha256()
    with path.open('rb') as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            digest.update(block)
    files.append({'path': str(path.relative_to(runtime)), 'bytes': path.stat().st_size, 'sha256': digest.hexdigest()})
versions = {}
for name, flag in [('mutool', '-v'), ('qpdf', '--version'), ('gs', '--version'), ('tesseract', '--version')]:
    result = subprocess.run([str(runtime / 'bin' / name), flag], text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, check=True)
    versions[name] = result.stdout.splitlines()[0]
manifest = {'platform': 'macos-arm64', 'components': versions,
            'nativeSources': json.loads((root / 'scripts/native-sources.json').read_text()),
            'files': files, 'totalBytes': sum(item['bytes'] for item in files)}
(runtime / 'manifest.json').write_text(json.dumps(manifest, indent=2, ensure_ascii=False))
print(f'Runtime manifest: {len(files)} files, {manifest["totalBytes"] / 1_000_000:.2f} MB')
