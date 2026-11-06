#!/usr/bin/env python3
"""Build pinned macOS 14 arm64 workers; no Homebrew runtime libraries.

Place the archives listed in native-sources.json under work/runtime-sources.
Requires Xcode command line tools, CMake and make. Build tools are not bundled.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
BUILD = ROOT / 'work/runtime-build'
PREFIX = BUILD / 'prefix'
RUNTIME = ROOT / 'src-tauri/resources/runtime'
SOURCES = json.loads((ROOT / 'scripts/native-sources.json').read_text())
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--sources', type=Path, default=ROOT / 'work/runtime-sources')
parser.add_argument('--install', action='store_true', help='Replace bundled workers after building and auditing')
args = parser.parse_args()
if sys.platform != 'darwin':
    parser.error('This build targets macOS Apple Silicon.')

def run(*command, cwd=ROOT, env=None):
    subprocess.run([str(v) for v in command], cwd=cwd, env=env, check=True)

def cmake(source, output, **options):
    config = dict(CMAKE_BUILD_TYPE='Release', CMAKE_OSX_DEPLOYMENT_TARGET='14.0',
                  CMAKE_OSX_ARCHITECTURES='arm64', CMAKE_INSTALL_PREFIX=PREFIX)
    config.update(options)
    run('cmake', '-S', source, '-B', output, *[f'-D{k}={v}' for k, v in config.items()])

BUILD.mkdir(parents=True, exist_ok=True)
for item in SOURCES:
    archive = args.sources / item['archive']
    if not archive.is_file() or hashlib.sha256(archive.read_bytes()).hexdigest() != item['sha256']:
        parser.error(f"Missing or invalid archive: {archive}. Source: {item['url']}")
    if not (BUILD / item['directory']).exists():
        run('tar', '-xf', archive, '-C', BUILD)

mu = BUILD / 'mupdf-1.28.6-source'
gs = BUILD / 'ghostpdl-10.08.0'
run(sys.executable, ROOT / 'scripts/build-mupdf-macos.py', args.sources / SOURCES[0]['archive'])
sdk = subprocess.check_output(['xcrun', '--show-sdk-path'], text=True).strip()
zlib = dict(ZLIB_LIBRARY=f'{sdk}/usr/lib/libz.tbd', ZLIB_INCLUDE_DIR=f'{sdk}/usr/include')
cmake(gs / 'libpng', BUILD / 'png-build', PNG_SHARED='OFF', PNG_TESTS='OFF', PNG_TOOLS='OFF', **zlib)
run('cmake', '--build', BUILD / 'png-build', '--parallel', 4)
run('cmake', '--install', BUILD / 'png-build')
cmake(BUILD / 'leptonica-1.87.0', BUILD / 'lept-build', BUILD_SHARED_LIBS='OFF', BUILD_PROG='OFF',
      ENABLE_GIF='OFF', ENABLE_TIFF='OFF', ENABLE_WEBP='OFF', ENABLE_OPENJPEG='OFF', ENABLE_JPEG='OFF',
      PNG_LIBRARY=PREFIX / 'lib/libpng16.a', PNG_PNG_INCLUDE_DIR=PREFIX / 'include', **zlib)
run('cmake', '--build', BUILD / 'lept-build', '--parallel', 4)
run('cmake', '--install', BUILD / 'lept-build')
cmake(BUILD / 'tesseract-5.5.4', BUILD / 'tess-build', BUILD_SHARED_LIBS='OFF', BUILD_TRAINING_TOOLS='OFF',
      BUILD_TESTS='OFF', GRAPHICS_DISABLED='ON', OPENMP_BUILD='OFF', DISABLE_ARCHIVE='ON',
      DISABLE_CURL='ON', DISABLE_TIFF='ON', ENABLE_NATIVE='OFF',
      Leptonica_DIR=PREFIX / 'lib/cmake/leptonica', CMAKE_PREFIX_PATH=PREFIX)
run('cmake', '--build', BUILD / 'tess-build', '--target', 'tesseract', '--parallel', 4)

# Use the same pinned IJG JPEG sources as MuPDF, with the ordinary memory manager.
run('cc', '-O2', '-arch', 'arm64', '-mmacosx-version-min=14.0',
    '-I' + str(mu / 'scripts/libjpeg'), '-I' + str(mu / 'thirdparty/libjpeg'),
    '-c', mu / 'thirdparty/libjpeg/jmemnobs.c', '-o', BUILD / 'jpeg-memory.o')
cmake(BUILD / 'qpdf-12.4.2', BUILD / 'qpdf-native14', BUILD_SHARED_LIBS='OFF', BUILD_STATIC_LIBS='ON',
      USE_IMPLICIT_CRYPTO='OFF', REQUIRE_CRYPTO_NATIVE='ON', BUILD_DOC='OFF', EXTERNAL_LIBS='ON',
      CMAKE_CXX_FLAGS=f'-I{mu}/thirdparty/libjpeg -I{mu}/scripts/libjpeg',
      CMAKE_EXE_LINKER_FLAGS=f'{mu}/build/release/libmupdf-third.a {BUILD}/jpeg-memory.o -lz')
run('cmake', '--build', BUILD / 'qpdf-native14', '--target', 'qpdf', '--parallel', 4)
env = dict(os.environ, CFLAGS='-O2 -arch arm64 -mmacosx-version-min=14.0',
           CXXFLAGS='-O2 -arch arm64 -mmacosx-version-min=14.0',
           LDFLAGS='-arch arm64 -mmacosx-version-min=14.0', PKG_CONFIG_LIBDIR='/nonexistent')
run('./configure', '--prefix=/opt/bpdf-runtime', '--without-x', '--disable-cups', '--disable-gtk',
    '--disable-fontconfig', '--without-tesseract', '--without-libidn', '--without-libpaper', '--without-ijs', cwd=gs, env=env)
run('make', '-j4', cwd=gs, env=env)

workers = {'mutool': mu / 'build/release/mutool', 'bpdf-render': BUILD / 'bpdf-render',
           'qpdf': BUILD / 'qpdf-native14/qpdf/qpdf', 'gs': gs / 'bin/gs',
           'tesseract': BUILD / 'tess-build/bin/tesseract'}
for name, binary in workers.items():
    run('vtool', '-show-build', binary)
    dependencies = subprocess.check_output(['otool', '-L', str(binary)], text=True).splitlines()[1:]
    if any(not line.strip().startswith(('/usr/lib/', '/System/Library/')) for line in dependencies):
        raise SystemExit(f'Non-system runtime dependency in {name}: {dependencies}')
    if args.install:
        target = RUNTIME / 'bin' / name
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(binary, target)
        run('strip', '-x', target)
        run('codesign', '--force', '--sign', '-', target)

if args.install:
    # Keep source-specific licensing, including vendored components, alongside the runtime.
    for item in SOURCES:
        source = BUILD / item['directory']
        for path in source.rglob('*'):
            if path.is_file() and path.name.upper().startswith(('LICENSE', 'COPYING', 'NOTICE')):
                target = RUNTIME / 'licenses/native-sources' / item['directory'] / path.relative_to(source)
                target.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(path, target)
    shutil.copyfile(ROOT / 'scripts/native-sources.json', RUNTIME / 'licenses/native-sources.json')
print('Build complete. Run engine, Office, OCR, PDF/A and full bundle compatibility checks before publishing.')
