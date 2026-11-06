#!/usr/bin/env python3
"""Rebuild the bundled PDF engine for macOS 14 without Homebrew dylibs.

Provide the official MuPDF 1.28.6 source archive; outputs stay in work until
the engine integration suite is run against the replacement runtime.
"""
import argparse
import hashlib
import pathlib
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
SHA256 = 'dc85000640a45d90ca9b77b061f6ca67cce0c4d4fbecf8819647baf573eef101'
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('archive', type=pathlib.Path)
args = parser.parse_args()
if sys.platform != 'darwin':
    parser.error('Requires macOS and the Xcode command line tools.')
archive = args.archive.resolve()
if hashlib.sha256(archive.read_bytes()).hexdigest() != SHA256:
    parser.error('Source archive SHA-256 does not match MuPDF 1.28.6.')
build = ROOT / 'work/runtime-build'
build.mkdir(parents=True, exist_ok=True)
source = build / 'mupdf-1.28.6-source'
if not source.exists():
    subprocess.run(['tar', '-xzf', str(archive), '-C', str(build)], check=True)
subprocess.run([
    'make', '-j4', 'build=release', 'HAVE_X11=no', 'HAVE_GLUT=no',
    'HAVE_LIBCRYPTO=no', 'USE_SYSTEM_LIBS=no', 'USE_TESSERACT=no',
    'USE_LIBARCHIVE=no', 'USE_CURL=no',
    'ARCHFLAGS=-arch arm64 -mmacosx-version-min=14.0',
    'XCFLAGS=-mmacosx-version-min=14.0', 'XLDFLAGS=-mmacosx-version-min=14.0',
    'build/release/mutool',
], cwd=source, check=True)
subprocess.run([
    'cc', '-O2', '-arch', 'arm64', '-mmacosx-version-min=14.0',
    '-I' + str(source / 'include'), str(ROOT / 'native/reader.c'),
    '-L' + str(source / 'build/release'), '-lmupdf', '-lmupdf-third',
    '-lc++', '-lm', '-lpthread', '-o', str(build / 'bpdf-render'),
], check=True)
for binary in (source / 'build/release/mutool', build / 'bpdf-render'):
    subprocess.run(['vtool', '-show-build', str(binary)], check=True)
    subprocess.run(['otool', '-L', str(binary)], check=True)
