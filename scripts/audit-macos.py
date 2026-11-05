#!/usr/bin/env python3
"""Inspect actual Mach-O deployment targets; never infer compatibility from Info.plist."""
import argparse,json,re,subprocess,sys
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('path',type=Path);p.add_argument('--target',default='14.0');p.add_argument('--report',type=Path,required=True);p.add_argument('--strict',action='store_true');a=p.parse_args()
def version(s):return tuple((list(map(int,s.split('.')))+[0,0])[:3])
files=[];issues=[];total=0
for f in a.path.rglob('*'):
 if not f.is_file() or f.is_symlink():continue
 total+=f.stat().st_size
 with f.open('rb') as h:magic=h.read(4)
 if magic not in [b'\xcf\xfa\xed\xfe',b'\xfe\xed\xfa\xcf',b'\xca\xfe\xba\xbe',b'\xbe\xba\xfe\xca']:continue
 out=subprocess.run(['otool','-l',str(f)],capture_output=True,text=True,check=True).stdout
 targets=re.findall(r'\bminos\s+(\d+(?:\.\d+)+)',out)+re.findall(r'cmd LC_VERSION_MIN_MACOSX\s+cmdsize \d+\s+version (\d+(?:\.\d+)+)',out)
 if not targets:issues.append({'path':str(f.relative_to(a.path)),'reason':'Missing deployment target'})
 for target in targets:
  files.append({'path':str(f.relative_to(a.path)),'minimumOS':target})
  if version(target)>version(a.target):issues.append({'path':str(f.relative_to(a.path)),'reason':f'Requires macOS {target}, exceeds {a.target}'})
report={'target':a.target,'installedFileBytes':total,'within700MB':total<=700_000_000,'compatible':not issues,'binaries':files,'issues':issues}
a.report.parent.mkdir(parents=True,exist_ok=True);a.report.write_text(json.dumps(report,indent=2))
print(f'{total/1e6:.2f} MB; {len(files)} Mach-O records; {len(issues)} compatibility failures')
if a.strict and issues:sys.exit(1)
