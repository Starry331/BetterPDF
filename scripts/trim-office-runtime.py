#!/usr/bin/env python3
"""Compact LibreOffice layout XML without deleting resources or changing values.

Even headless conversions load window resources. Preserve all nodes, comments,
attributes and property text; remove indentation only in structural containers.
"""
import argparse,json
from pathlib import Path
from xml.parsers import expat
CONTAINERS={'interface','object','child','packing','attributes','items','style','action-widgets','accessibility'}
def compact(data):
    if b'<!DOCTYPE' in data or b'<!ENTITY' in data:raise ValueError('Unsupported layout XML entities')
    parser=expat.ParserCreate();stack=[];ranges=[]
    parser.StartElementHandler=lambda name,attrs:stack.append(name)
    parser.EndElementHandler=lambda name:stack.pop()
    def whitespace(text):
        if stack and stack[-1] in CONTAINERS and text.isspace():
            start=parser.CurrentByteIndex;end=start
            # Expat normalizes CRLF and may split whitespace callbacks. Work on
            # raw byte boundaries; leave entity references exactly as written.
            while end<len(data) and data[end] in b' \t\r\n':end+=1
            if end>start:ranges.append((start,end))
    parser.CharacterDataHandler=whitespace;parser.Parse(data,True)
    result=[];at=0
    for start,end in ranges:
        if end<=at:continue
        result.append(data[at:max(at,start)]);at=end
    result.append(data[at:]);return b''.join(result)
def trim(runtime:Path,apply=False):
    runtime=runtime.resolve();config=runtime/'office/Contents/Resources/config/soffice.cfg';files=[]
    for path in sorted(config.rglob('*.ui')):
        if path.is_symlink() or 'ui' not in path.relative_to(config).parts[:-1]:continue
        if not path.resolve().is_relative_to(runtime):raise ValueError('Runtime path escapes bundle')
        before=path.read_bytes();after=compact(before)
        if len(after)>=len(before):continue
        files.append({'path':str(path.relative_to(runtime)),'before':len(before),'after':len(after)})
        if apply:path.write_bytes(after)
    return {'savedBytes':sum(f['before']-f['after'] for f in files),'files':files,'applied':apply}
if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('runtime',type=Path);parser.add_argument('--apply',action='store_true');parser.add_argument('--report',type=Path)
    args=parser.parse_args();report=trim(args.runtime,args.apply)
    if args.report:args.report.write_text(json.dumps(report,indent=2))
    print(f"{'Saved' if args.apply else 'Can save'} {report['savedBytes']/1_000_000:.2f} MB in {len(report['files'])} layout files")
