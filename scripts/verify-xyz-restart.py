"""Recheck actual XYZ files after restarting the native server; never refetch."""
import argparse,base64,hashlib,json,pathlib,urllib.request

def main():
    p=argparse.ArgumentParser();p.add_argument('--server',default='http://127.0.0.1:4370');p.add_argument('--out',default='.verification/xyz-public');a=p.parse_args();out=pathlib.Path(a.out)
    before=json.loads((out/'runtime.json').read_text('utf-8'));ui=json.loads((out/'ui-acquisition.json').read_text('utf-8'))
    def get(path):return urllib.request.urlopen(a.server+path,timeout=20).read()
    def post(path,payload):return json.load(urllib.request.urlopen(urllib.request.Request(a.server+path,data=json.dumps(payload).encode(),headers={'Content-Type':'application/json','X-GeoD-Client':'geod-global'}),timeout=20))
    assert len(json.loads(get('/map-services')))==2 and len(json.loads(get('/map-images')))==3
    checked=[]
    original=json.loads(get('/proxy'));post('/proxy',{'mode':'custom','url':'http://127.0.0.1:9'})
    try:
        for case in before['cases']:
            asset=json.loads((out/(case['kind']+'-asset.json')).read_text('utf-8'));inspection=json.loads(get('/map-images/'+case['imageId']));assert inspection['asset']==asset
            png=base64.b64decode(inspection['imageUrl'].split(',')[1]);assert png==(out/(case['kind']+'-image.png')).read_bytes()
            package=get('/map-images/'+case['imageId']+'/export');assert package==(out/(case['kind']+'.zip')).read_bytes();checked.append(dict(imageId=case['imageId'],metadataExact=True,pngExact=True,exportExact=True))
        inspection=json.loads(get('/map-images/'+ui['asset']['id']));assert inspection['asset']==ui['asset'];png=base64.b64decode(inspection['imageUrl'].split(',')[1]);assert hashlib.sha256(png).hexdigest()==ui['asset']['sha256']
        package=get('/map-images/'+ui['asset']['id']+'/export');assert package==(out/'ui.zip').read_bytes();checked.append(dict(imageId=ui['asset']['id'],metadataExact=True,pngExact=True,exportExact=True))
    finally:post('/proxy',original)
    report={'passed':True,'mode':'Actual native process restart, rejecting proxy, only local inspect/export calls','networkNeededForRead':False,'usedUserDesktop':False,'images':checked}
    (out/'restart.json').write_text(json.dumps(report,indent=2),encoding='utf-8');print(json.dumps({'passed':True,'images':len(checked)}))
if __name__=='__main__':main()
