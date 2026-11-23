"""Check the macOS icon declaration and ICNS bytes before packaging."""
import plistlib
import struct
from pathlib import Path


def check_app_icon(app: Path, expected: Path):
    errors = []
    with (app / 'Contents/Info.plist').open('rb') as stream:
        info = plistlib.load(stream)
    name = info.get('CFBundleIconFile', '')
    if not isinstance(name, str) or not name:
        return '', ['Missing CFBundleIconFile declaration']
    icon = app / 'Contents/Resources' / name
    if not icon.is_file() or not icon.resolve().is_relative_to(app.resolve()):
        return name, ['Missing or external application icon']
    data = icon.read_bytes()
    if len(data) < 8 or data[:4] != b'icns' or struct.unpack('>I', data[4:8])[0] != len(data):
        errors.append('Invalid ICNS header or file length')
    if data != expected.read_bytes():
        errors.append('Application icon differs from source asset')
    return name, errors
