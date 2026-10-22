"""Create a zstd dictionary delta, verify exact reconstruction, and extend a signed update manifest.

The patch is not an executable. Clients still require the reconstructed complete
installer's version-bound Minisign signature before any installation.
Requires the publisher-only python-zstandard package, never installed by the app.
"""
import argparse, hashlib, json, re
from pathlib import Path
from urllib.parse import urlparse

def digest(data):
    return hashlib.sha256(data).hexdigest()

def build(base, target):
    import zstandard as zstd
    dictionary = zstd.ZstdCompressionDict(base, dict_type=zstd.DICT_TYPE_RAWCONTENT)
    parameters = zstd.ZstdCompressionParameters.from_level(12, window_log=29, enable_ldm=True)
    patch = zstd.ZstdCompressor(dict_data=dictionary, compression_params=parameters).compress(target)
    restored = zstd.ZstdDecompressor(dict_data=dictionary, max_window_size=512*1024).decompress(patch, max_output_size=len(target))
    if restored != target:
        raise ValueError('Delta reconstruction did not match the installer')
    return patch

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    for name in ['base','installer','manifest','output']:parser.add_argument('--'+name,required=True,type=Path)
    parser.add_argument('--from-version',required=True);parser.add_argument('--url',required=True)
    args=parser.parse_args()
    url=urlparse(args.url)
    if url.scheme!='https' or not url.hostname or url.username or url.password or url.fragment:raise ValueError('Use a public HTTPS patch URL')
    if not re.fullmatch(r'\d+\.\d+\.\d+(?:-[\w.-]+)?',args.from_version):raise ValueError('Invalid base version')
    manifest=json.loads(args.manifest.read_text(encoding='utf-8'))
    if manifest.get('product')!='xyz.laogao.geod.global' or manifest.get('version')==args.from_version:raise ValueError('Invalid Global target')
    for path in [args.base,args.installer]:
        if not 0<path.stat().st_size<=512*1024*1024:raise ValueError('Installer outside delta size bounds')
    base=args.base.read_bytes();target=args.installer.read_bytes()
    patch=build(base,target)
    args.output.parent.mkdir(parents=True,exist_ok=True);args.output.write_bytes(patch)
    worthwhile=len(patch)<len(target)*0.85
    metadata={'algorithm':'zstd-dictionary-v1','url':args.url,'bytes':len(patch),'sha256':digest(patch),
              'baseBytes':len(base),'baseSha256':digest(base),'targetBytes':len(target),'targetSha256':digest(target)}
    if worthwhile:
        manifest.setdefault('deltas',{})[args.from_version]=metadata
        args.manifest.write_text(json.dumps(manifest,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
    receipt={**metadata,'fromVersion':args.from_version,'toVersion':manifest['version'],'offered':worthwhile,
             'restoredSha256':digest(target),'savedPercent':round((1-len(patch)/len(target))*100,2)}
    args.output.with_suffix('.receipt.json').write_text(json.dumps(receipt,indent=2)+'\n',encoding='utf-8')
    print(json.dumps(receipt))

if __name__=='__main__':main()
