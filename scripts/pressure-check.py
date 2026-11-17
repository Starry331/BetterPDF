#!/usr/bin/env python3
"""Bounded stress of bundled workers, including live reader probes and job cancellation.
RSS is sampled for the harness and its complete descendant tree; not GUI memory.
"""
import hashlib, json, os, platform, signal, subprocess, time
from pathlib import Path
root=Path(__file__).resolve().parents[1]
work=root/'work/pressure';work.mkdir(parents=True,exist_ok=True)
resources=Path(os.environ.get('BPDF_RESOURCES',root/'src-tauri/target/release/bundle/macos/BPDF.app/Contents/Resources/resources'))
env=dict(os.environ,BPDF_RESOURCES=str(resources),PATH='/usr/bin:/bin:/usr/sbin:/sbin')
cli=root/'src-tauri/target/debug/bpdf-cli'
source=Path(os.environ.get('BPDF_STRESS_SOURCE',root/'work/stress/1000-pages.pdf'))
assert source.is_file(), 'Run scripts/stress-check.py first'
def digest(path):
    h=hashlib.sha256()
    with path.open('rb') as stream:
        for block in iter(lambda:stream.read(1024*1024),b''):h.update(block)
    return h.hexdigest()
source_hash=digest(source)
results=[];sequence=0

def tree(pid):
    rows=[]
    for line in subprocess.check_output(['/bin/ps','-axo','pid=,ppid=,rss='],text=True).splitlines():
        values=line.split()
        if len(values)==3:rows.append(tuple(map(int,values)))
    ids={pid}
    while True:
        expanded=ids|{p for p,parent,_ in rows if parent in ids}
        if expanded==ids:break
        ids=expanded
    return ids,sum(rss*1024 for p,_,rss in rows if p in ids)

def call(name,request,expected=True,probe=False,timeout=240):
    global sequence
    sequence+=1
    req=work/f'{sequence}-request.json';req.write_text(json.dumps(request))
    stdout=work/f'{sequence}-stdout.json';stderr=work/f'{sequence}-stderr.txt'
    peak=0;latencies=[];started=time.monotonic();next_probe=started+.2
    with stdout.open('w') as out,stderr.open('w') as err:
        process=subprocess.Popen([cli,req],env=env,stdout=out,stderr=err,start_new_session=True)
        try:
            while process.poll() is None:
                ids,rss=tree(process.pid);peak=max(peak,rss)
                if rss>2*1024**3 or time.monotonic()-started>timeout:
                    raise RuntimeError('Stress guard: 2 GiB process tree or time budget exceeded')
                if probe and time.monotonic()>=next_probe:
                    reader=work/'reader.json';reader.write_text(json.dumps({'action':'page','input':str(source),'page':999,'includeChars':False}))
                    then=time.monotonic();r=subprocess.run([cli,reader],env=env,capture_output=True,text=True,timeout=15)
                    assert r.returncode==0 and json.loads(r.stdout)['ok'],r.stderr
                    latencies.append(round(time.monotonic()-then,3));next_probe=time.monotonic()+.3
                time.sleep(.05)
            value=json.loads(stdout.read_text());assert value['ok']==expected,value
            assert process.returncode==(0 if expected else 1), 'Unexpected crash/exit status'
            if not expected:assert value.get('error')=='CANCELLED',value
        finally:
            if process.poll() is None:
                ids,_=tree(process.pid)
                for pid in sorted(ids,reverse=True):
                    try:os.kill(pid,signal.SIGKILL)
                    except ProcessLookupError:pass
                process.wait()
    result={'name':name,'passed':True,'seconds':round(time.monotonic()-started,3),'sampledPeakProcessTreeBytes':peak,'readerProbeSeconds':latencies,'exitCode':process.returncode}
    results.append(result);print(json.dumps(result),flush=True)
    return value

def job(name,tool,inputs,options={},cancel=None,probe=False):
    output=work/f'{name}.pdf';output.unlink(missing_ok=True)
    request={'mode':'job','tool':tool,'inputs':list(map(str,inputs)),'output':str(output),'options':options}
    if cancel is not None:request['cancelAfterMs']=cancel
    call(name,request,cancel is None,probe)
    if cancel is not None:assert not output.exists(),'Cancelled job published partial output'
    else:
        assert output.is_file() and output.stat().st_size>0
        call(name+'-reopen',{'action':'info','input':str(output)})
    return output

job('rotate-1000','rotate',[source],{'angle':90},probe=True)
job('compress-1000','compress',[source],probe=True)
short=job('extract-12','extract',[source],{'pages':'1-12'})
job('rasterize-12','rasterize',[short],{'dpi':96},probe=True)
full=job('rasterize-1000','rasterize',[source],{'dpi':72},probe=True)
assert call('verify-1000-raster-pages',{'action':'info','input':str(full)})['result']['pages']==1000
job('cancel-1000-ocr','ocr',[source],{'dpi':144,'language':'eng'},cancel=700,probe=True)
job('cancel-1000-raster','rasterize',[source],{'dpi':144},cancel=700,probe=True)
for i in range(30):
    page=(i*379)%1000
    output=work/'repeat.png'
    call(f'repeated-render-{i}',{'action':'render','input':str(source),'page':page,'scale':1.25,'output':str(output)})
    output.unlink(missing_ok=True)
assert digest(source)==source_hash,'Source changed'
report={'platform':platform.platform(),'resources':str(resources),'inputBytes':source.stat().st_size,'pages':1000,'scope':'Bounded worker stress with packaged resources and minimal PATH; sampled total descendant RSS, not GUI or baseline hardware; all subprocesses exit between repeated renders','originalUnchanged':True,'results':results}
(work/'results.json').write_text(json.dumps(report,indent=2))
