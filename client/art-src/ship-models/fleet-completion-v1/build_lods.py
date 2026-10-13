from concurrent.futures import ThreadPoolExecutor
from pathlib import Path
import subprocess,json,os
from scaffold import SPECS,ROOT,MODELS
lab=MODELS/'interceptor-lod-experiment-v1'
env=dict(os.environ,NODE_PATH='/private/tmp/stellar-interceptor-tools/node_modules')
def build(ship):
    output=[]
    for command in [('node','build_models.mjs',ship),('python3','verify_geometry.py',ship)]:
        result=subprocess.run(command,cwd=lab,env=env,text=True,stdout=subprocess.PIPE,stderr=subprocess.STDOUT)
        output.append(result.stdout)
        if result.returncode:
            (ROOT/(ship+'-lod-build.log')).write_text('\n'.join(output))
            return {'ship':ship,'failed':command,'output':result.stdout[-3500:]}
    (ROOT/(ship+'-lod-build.log')).write_text('\n'.join(output))
    info=json.loads((lab/(ship+'-model-info.json')).read_text())
    return {'ship':ship,'levels':[{k:l[k] for k in ('level','triangles','drawCalls')} for l in info['levels']]}
with ThreadPoolExecutor(max_workers=3) as pool:results=list(pool.map(build,SPECS))
(ROOT/'lod-build-results.json').write_text(json.dumps(results,indent=2)+'\n')
print(json.dumps(results,indent=2))
if any('failed' in r for r in results):raise SystemExit(1)
