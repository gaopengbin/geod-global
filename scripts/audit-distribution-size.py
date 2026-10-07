"""Measure owned files and current Release metadata; never creates an installer."""
import argparse,json,subprocess
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]

def size(directory):
    return sum(path.stat().st_size for path in directory.rglob('*') if path.is_file()) if directory.exists() else 0

def audit(release=None):
    runtime=ROOT/'.agent-runtime/win32-x64'
    manifest=json.loads((runtime/'manifest.json').read_text(encoding='utf-8'))
    files=manifest['files']
    groups={
        'Codex engine':files['codex.exe']['bytes'],
        'Node runtime':files['node.exe']['bytes'],
        'Agent adapter':files['agent.mjs']['bytes'],
        'Runtime notices':sum(item['bytes'] for name,item in files.items() if name not in ('codex.exe','node.exe','agent.mjs')),
    }
    frontend={folder.name:size(folder) for folder in (ROOT/'prototype/dist').iterdir() if folder.is_dir()}
    result={'schemaVersion':1,'unit':'bytes','installerCreated':False,'agentRuntime':groups,'agentRuntimeTotal':sum(groups.values()),'frontendBeforeEmbedding':frontend,
        'excludedRuntimeGroups':['Java/JRE','local OCR','Python/GDAL environment','Codex shell helpers','Codex voice/code-mode hosts','npm node_modules','Cesium runtime assets'],
        'measurementBoundary':'Frontend resources are compressed and embedded by Tauri. Development executables and raw runtime sizes are not installer download sizes.'}
    if release:
        data=json.loads(subprocess.check_output(['gh','release','view',release,'--repo','gaopengbin/geod-global','--json','tagName,assets'],text=True,encoding='utf-8'))
        result['publishedRelease']={'version':data['tagName'],'assets':[{'name':a['name'],'bytes':a['size'],'url':a['url']} for a in data['assets'] if a['name'].endswith(('.exe','.zip'))]}
    return result

if __name__=='__main__':
    parser=argparse.ArgumentParser();parser.add_argument('--release');parser.add_argument('--output',type=Path,default=ROOT/'prototype/qa/distribution-size-audit.json');args=parser.parse_args()
    result=audit(args.release);args.output.parent.mkdir(parents=True,exist_ok=True);args.output.write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8');print(json.dumps(result,indent=2))
