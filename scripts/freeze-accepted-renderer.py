"""Freeze a passed production-renderer receipt and its development desktop build.
Private local evidence only; never makes an installer or changes canonical binaries.
"""
import argparse, hashlib, json, shutil
from pathlib import Path
def sha(path):
    h=hashlib.sha256()
    with Path(path).open('rb') as stream:
        for block in iter(lambda:stream.read(1024*1024),b''):h.update(block)
    return h.hexdigest()
def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--ui',required=True);p.add_argument('--root',required=True);p.add_argument('--desktop',default='.verification/naip-native-target/debug/geod-global-desktop.exe');a=p.parse_args()
    workspace=Path.cwd().resolve();root=Path(a.root).resolve();ui_path=Path(a.ui).resolve();desktop=Path(a.desktop).resolve();dist=workspace/'prototype/dist'
    assert root.parent==workspace/'.verification' and root.name.startswith('renderer-') and not root.exists()
    assert ui_path.is_relative_to(workspace/'.verification') and desktop.is_relative_to(workspace/'.verification')
    ui=json.loads(ui_path.read_text(encoding='utf-8'));assert ui['status']=='passed' and ui['errors']==[] and ui['remoteRequests']==[]
    for path,digest in ui['resources'].items():
        file=(workspace/path).resolve();assert file.is_relative_to(dist.resolve()) and sha(file)==digest
    root.mkdir();shutil.copytree(dist,root/'dist');digest=sha(desktop);frozen=root/f'desktop-{digest[:16]}.exe';shutil.copy2(desktop,frozen);assert sha(frozen)==digest
    files=[]
    for file in sorted((root/'dist').rglob('*')):
        if file.is_file():
            relative=file.relative_to(root/'dist');assert sha(file)==sha(dist/relative)
            files.append({'path':str(relative).replace('\\','/'),'bytes':file.stat().st_size,'sha256':sha(file)})
    record={'schema':'geod-accepted-renderer/v1','distFiles':files,'uiReceiptSha256':sha(ui_path),'desktop':{'path':str(frozen),'bytes':frozen.stat().st_size,'sha256':digest}}
    (root/'receipt.json').write_text(json.dumps(record,indent=2)+'\n',encoding='utf-8');print(json.dumps({'status':'passed','files':len(files),'desktop':record['desktop']}))
if __name__=='__main__':main()
