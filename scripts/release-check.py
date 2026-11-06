#!/usr/bin/env python3
"""Check evidence for the requested platform; never infer test success from files alone."""
import argparse,json,sys
from pathlib import Path
root=Path(__file__).resolve().parents[1]
p=argparse.ArgumentParser()
p.add_argument('--platform',choices=['macos','windows','both'],default='macos')
p.add_argument('--signed',action='store_true',help='Require distribution signing/notarization evidence')
a=p.parse_args()
required=['all34SuccessFailureCases','veraPDFA2b','chineseOCR','traditionalChineseOCR','cajHNCorpus','performanceBaseline','dependencySourceMaterials','workerFilesystemIsolation','officeCompatibility']
if a.platform in ('macos','both'):
 required+=['macos14CleanInstall']
 if a.signed:required+=['macosSignedNotarized']
if a.platform in ('windows','both'):
 required+=['windows11CleanInstall']
 if a.signed:required+=['windowsSigned']
evidence=root/'release-evidence.json'
data=json.loads(evidence.read_text()) if evidence.exists() else {}
def valid(key):
 item=data.get(key)
 if not isinstance(item,dict) or item.get('passed') is not True or not isinstance(item.get('report'),str):return False
 report=(root/item['report']).resolve()
 return report.is_relative_to(root) and report.is_file()
missing=[key for key in required if not valid(key)]
if missing:
 print('Release blocked. Missing independently reviewable evidence:\n'+'\n'.join('- '+key for key in missing));sys.exit(1)
print(f'Evidence inventory complete for {a.platform} ({"signed" if a.signed else "unsigned download"}). Review report contents before publishing.')
