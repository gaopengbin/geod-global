"""Prepare public Global update configuration and sign metadata after a build.

Never builds, installs, publishes, or reads another application's signing key.
The signing subprocess alone receives GEOD_GLOBAL_SIGNING_PRIVATE_KEY.
"""
import argparse,base64,datetime,hashlib,json,os,re,subprocess
from pathlib import Path
from urllib.parse import urlparse
ROOT=Path(__file__).resolve().parents[1]
PRODUCT='xyz.laogao.geod.global'

def https(raw):
    url=urlparse(raw)
    return url.scheme=='https' and url.hostname and not url.username and not url.password and not url.fragment

def public_channel(value):
    if set(value)!={'product','endpoint','pubkey','messagesEndpoint'} or value['product']!=PRODUCT or not https(value['endpoint']) or value['messagesEndpoint'] and not https(value['messagesEndpoint']):
        raise ValueError('Use the independent Global product identity and HTTPS endpoints.')
    public=base64.b64decode(value['pubkey'],validate=True).decode('utf-8')
    lines=public.strip().splitlines()
    if len(lines)!=2 or not lines[0].startswith('untrusted comment:') or len(base64.b64decode(lines[1],validate=True))!=42:
        raise ValueError('A complete official Tauri public key is required.')
    return value

def write(path,value):
    path.parent.mkdir(parents=True,exist_ok=True)
    path.write_text(json.dumps(value,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')

def sign(path,version=None):
    key=os.environ.get('GEOD_GLOBAL_SIGNING_PRIVATE_KEY')
    if not key:raise ValueError('The independent Global signing key is required for this post-build action.')
    env=os.environ.copy()
    for name in list(env):
        if name.startswith(('TAURI_SIGNING_PRIVATE_KEY','GEOD_GLOBAL_SIGNING_PRIVATE_KEY')):env.pop(name)
    env['TAURI_SIGNING_PRIVATE_KEY']=key
    env['TAURI_SIGNING_PRIVATE_KEY_PASSWORD']=os.environ.get('GEOD_GLOBAL_SIGNING_PASSWORD','')
    args=['node',str(ROOT/'node_modules/@tauri-apps/cli/tauri.js'),'signer','sign']
    if version:args+=['--app-version',version]
    # Capture and discard CLI diagnostics, which must never echo key material.
    result=subprocess.run([*args,str(path.resolve())],cwd=ROOT,env=env,capture_output=True)
    if result.returncode:raise RuntimeError('Official Tauri signing failed. No key or signing diagnostics were saved.')
    return path.with_suffix(path.suffix+'.sig').read_text(encoding='utf-8').strip()

def update_manifest(version,signature,url,notes):
    if not re.fullmatch(r'(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-[0-9A-Za-z.-]+)?',version) or not https(url) or len(notes)>12000:
        raise ValueError('Valid version, HTTPS installer URL and bounded release notes are required.')
    comment=base64.b64decode(signature,validate=True).decode('utf-8').splitlines()[2]
    if not comment.startswith('trusted comment: ') or 'version:'+version not in comment[len('trusted comment: '):].split('\t'):
        raise ValueError('The signature must be bound to exactly this version.')
    return {'product':PRODUCT,'version':version,'notes':notes,'pub_date':datetime.datetime.now(datetime.timezone.utc).isoformat(),
            'platforms':{'windows-x86_64':{'url':url,'signature':signature}}}

def main():
    parser=argparse.ArgumentParser(description=__doc__);sub=parser.add_subparsers(dest='command',required=True)
    config=sub.add_parser('prepare-build');config.add_argument('--channel',required=True,type=Path);config.add_argument('--output',required=True,type=Path)
    update=sub.add_parser('sign-update');update.add_argument('--installer',required=True,type=Path);update.add_argument('--version',required=True);update.add_argument('--url',required=True);update.add_argument('--notes',required=True,type=Path);update.add_argument('--output',required=True,type=Path)
    news=sub.add_parser('sign-notifications');news.add_argument('--source',required=True,type=Path);news.add_argument('--output',required=True,type=Path)
    args=parser.parse_args()
    if args.command=='prepare-build':
        value=public_channel(json.loads(args.channel.read_text(encoding='utf-8')))
        args.output.parent.mkdir(parents=True,exist_ok=True)
        args.output.write_text('GEOD_GLOBAL_DISTRIBUTION='+json.dumps(value,separators=(',',':'))+'\n',encoding='utf-8')
    elif args.command=='sign-update':
        with args.installer.open('rb') as installer:
            if installer.read(2)!=b'MZ':raise ValueError('Expected an actual Windows installer.')
        notes=args.notes.read_text(encoding='utf-8');signature=sign(args.installer,args.version)
        write(args.output,update_manifest(args.version,signature,args.url,notes))
        write(args.output.with_name('update-signing-receipt.json'),{'product':PRODUCT,'version':args.version,'installerBytes':args.installer.stat().st_size,'installerSha256':hashlib.file_digest(args.installer.open('rb'),'sha256').hexdigest(),'published':False})
    else:
        if args.source.stat().st_size>300000:raise ValueError('Notification feed is too large.')
        raw=args.source.read_bytes();feed=json.loads(raw)
        if feed.get('schemaVersion')!=1 or feed.get('product')!=PRODUCT or not isinstance(feed.get('items'),list) or len(feed['items'])>100:raise ValueError('Expected a bounded Global notification feed.')
        signature=sign(args.source)
        write(args.output,{'payload':base64.b64encode(raw).decode(),'signature':signature})
    print(json.dumps({'prepared':True,'command':args.command,'published':False}))

if __name__=='__main__':
    try:main()
    except Exception as failure:
        print(json.dumps({'prepared':False,'error':str(failure)}));raise SystemExit(1)
