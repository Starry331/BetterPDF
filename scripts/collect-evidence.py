#!/usr/bin/env python3
"""Collect narrow, inspectable test evidence without approving untested release gates."""
import hashlib
import json
import re
from pathlib import Path
import xml.etree.ElementTree as ET

root = Path(__file__).resolve().parents[1]
out = root / 'docs/verification'
out.mkdir(exist_ok=True)

def read(path):
    return json.loads((root / path).read_text())

def digest(path):
    return hashlib.sha256((root / path).read_bytes()).hexdigest()

def write(name, report):
    path = out / name
    path.write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n')
    return str(path.relative_to(root))

engine = read('work/integration-results.json')
cases = {case['name']: case['ok'] is True for case in engine['results']}
success = {
    'merge':'merge', 'split':'split', 'rotate':'rotate', 'delete':'delete',
    'extract':'extract and descending ranges', 'reorder':'reorder with duplicates',
    'resize':'resize', 'crop':'crop', 'to_docx':'PDF to DOCX', 'to_pptx':'PDF to PPTX',
    'to_xlsx':'PDF to XLSX', 'to_images':'PDF to images', 'to_pdfa':'PDF/A-2b generation',
    'to_txt':'PDF to TXT', 'to_md':'PDF to Markdown', 'to_epub':'PDF to EPUB',
    'word_pdf':'Word to PDF roundtrip', 'ppt_pdf':'PowerPoint to PDF roundtrip',
    'excel_pdf':'Excel to PDF roundtrip', 'images_pdf':'images to PDF', 'epub_pdf':'EPUB to PDF',
    'caj_pdf':'CAJ recovery', 'file_pdf':'generic file to PDF', 'compress':'lossless compression',
    'repair':'repair', 'ocr':'OCR with bundled English model', 'grayscale':'grayscale',
    'watermark':'add watermark', 'remove_watermark':'remove independent watermark',
    'protect':'AES-256 protection', 'unlock':'unlock', 'rasterize':'rasterize',
    'flatten':'flatten fields', 'file_images':'generic file to images',
}
tool_list = re.search(r'pub const TOOLS:.*?=&\[(.*?)\];', (root/'src-tauri/src/jobs.rs').read_text(), re.S)
assert tool_list and set(re.findall(r'"([a-z_]+)"', tool_list[1])) == set(success)
failure = 'all 34 tools reject missing and unsupported inputs without output'
matrix = [{'tool':tool, 'successCase':case, 'successPassed':cases.get(case, False),
           'failureCase':failure, 'failurePassed':cases.get(failure, False)} for tool,case in success.items()]
matrix_path = write('tool-matrix.json', {'scope':'Basic successful fixtures and input-rejection cases; not arbitrary-document compatibility',
    'testSourceSha256':digest('tests/engine.integration.mjs'), 'reportSha256':digest('work/integration-results.json'), 'tools':matrix})
evidence = {'all34SuccessFailureCases':{'passed':all(x['successPassed'] and x['failurePassed'] for x in matrix), 'report':matrix_path}}
for key, case in [('chineseOCR','OCR Simplified Chinese scanned page'),('traditionalChineseOCR','OCR Traditional Chinese scanned page')]:
    path = write(key+'.json', {'scope':'Bundled language model on the synthetic scanned fixture', 'case':case, 'passed':cases.get(case, False), 'reportSha256':digest('work/integration-results.json')})
    evidence[key] = {'passed':cases.get(case, False), 'report':path}

pdfa = read('work/verapdf/results.json')
checks = []
for item in pdfa['results']:
    tree = ET.parse(root / item['report'])
    reports = [element for element in tree.iter() if element.tag.split('}')[-1] == 'validationReport']
    passed = bool(reports) and all(element.attrib.get('isCompliant') == 'true' for element in reports)
    checks.append({'sample':item['sample'], 'passed':passed, 'xmlSha256':digest(item['report'])})
path = write('pdfa.json', {'validator':pdfa['validator'], 'profile':pdfa['profile'], 'scope':'Previously validated standard and Chinese PDF/A samples; not every output', 'results':checks})
evidence['veraPDFA2b'] = {'passed':len(checks)==3 and all(x['passed'] for x in checks), 'report':path}

office = read('work/office-integration-results.json')
write('office.json', {'scope':'Implemented compatibility operations; full requested Office scope remains incomplete',
    'testSourceSha256':digest('tests/office.integration.mjs'), 'results':office['results']})
write('stress.json', read('work/stress/results.json'))
write('failure.json', read('work/failure-results.json'))
rust_log = (root/'work/completion-rust.log').read_text()
isolation_case = 'test engine::isolation_tests::worker_cannot_read_unselected_files_or_follow_escaping_symlinks ... ok'
isolation_ok = isolation_case in rust_log and '0 failed;' in rust_log
path = write('isolation.json', {'scope':'Selected files allowed; unselected reads, escaping symlinks and out-of-scope writes denied on this Mac', 'passed':isolation_ok, 'case':isolation_case, 'rustLogSha256':digest('work/completion-rust.log'), 'sourceSha256':digest('src-tauri/src/engine.rs')})
evidence['workerFilesystemIsolation'] = {'passed':isolation_ok, 'report':path}
pending = {
    'cajHNCorpus':'Multi-fragment/JBIG variants and representative authorized real files are incomplete.',
    'performanceBaseline':'The native smoke benchmark is not M1/8 GB GUI memory, startup or high-refresh acceptance.',
    'dependencySourceMaterials':'Complete redistribution source/build materials are not yet audited.',
    'officeCompatibility':'Complex Office object/table editing and native Office collaboration remain incomplete; 18 core compatibility cases passed.',
    'macos14CleanInstall':'No clean macOS 14 offline installation was available in this environment.',
}
for key, reason in pending.items():
    evidence[key] = {'passed':False, 'reason':reason}
(root/'release-evidence.json').write_text(json.dumps(evidence, ensure_ascii=False, indent=2)+'\n')
print(json.dumps({'verifiedGates':[k for k,v in evidence.items() if v['passed']], 'pending':pending}, ensure_ascii=False, indent=2))
