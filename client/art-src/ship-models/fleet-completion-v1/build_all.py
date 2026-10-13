"""Build independent packages with bounded parallelism; preserve existing ships."""
from pathlib import Path
from concurrent.futures import ThreadPoolExecutor
import subprocess
import json
import os
from scaffold import SPECS,ROOT,MODELS

env=dict(os.environ,NODE_PATH='/private/tmp/stellar-interceptor-tools/node_modules')
def build_one(ship):
    out=MODELS/(ship+'-textured-v1');log=[]
    for command in [('python3','prepare_textures.py'),('python3','build_model.py'),('node','validate.mjs'),('node','build_preview.mjs')]:
        result=subprocess.run(command,cwd=out,env=env,text=True,stdout=subprocess.PIPE,stderr=subprocess.STDOUT)
        log.append(' '.join(command)+'\n'+result.stdout)
        if result.returncode:
            (out/'build.log').write_text('\n'.join(log))
            return {'ship':ship,'failed':command,'output':result.stdout[-2000:]}
    (out/'build.log').write_text('\n'.join(log))
    info=json.loads((out/'model-info.json').read_text())
    return {'ship':ship,'triangles':info['triangles'],'sections':info['renderSections']}

if __name__=='__main__':
    with ThreadPoolExecutor(max_workers=3) as pool:
        results=list(pool.map(build_one,SPECS))
    (ROOT/'build-results.json').write_text(json.dumps(results,indent=2)+'\n')
    print(json.dumps(results,indent=2))
    if any('failed' in r for r in results):raise SystemExit(1)
