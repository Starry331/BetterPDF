#!/usr/bin/env python3
"""Fail closed until independent acceptance evidence is recorded for both platforms."""
import json,sys
from pathlib import Path
root=Path(__file__).resolve().parents[1]
evidence=root/'release-evidence.json'
required=['macos14CleanInstall','windows11CleanInstall','macosSignedNotarized','windowsSigned','all34SuccessFailureCases','veraPDFA2b','chineseOCR','traditionalChineseOCR','cajHNCorpus','performanceBaseline','dependencySourceMaterials','workerFilesystemIsolation']
data=json.loads(evidence.read_text()) if evidence.exists() else {}
missing=[key for key in required if not isinstance(data.get(key),dict) or data[key].get('passed') is not True or not data[key].get('report') or not (root/data[key]['report']).is_file()]
if missing:
 print('Release blocked. Missing independently reviewable evidence:\n'+'\n'.join('- '+key for key in missing));sys.exit(1)
print('Evidence inventory complete. Review reports before signing and publishing.')
