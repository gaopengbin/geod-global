"""Recheck persistent ArcGIS snapshots after the runtime has been restarted."""
import argparse,base64,hashlib,json,pathlib,urllib.request
def main():
 p=argparse.ArgumentParser();p.add_argument('--server',default='http://127.0.0.1:4369');p.add_argument('--out',default='.verification/arcgis-map-public');a=p.parse_args();out=pathlib.Path(a.out);before=json.loads((out/'runtime.json').read_text('utf-8'))
 def get(path):return urllib.request.urlopen(a.server+path,timeout=20).read()
 ui=json.loads((out/'ui-acquisition.json').read_text('utf-8'))
 services=json.loads(get('/map-services'));images=json.loads(get('/map-images'));assert len(services)==2 and len(images)==3
 checked=[]
 for case in before['cases']:
  asset=json.loads((out/(case['kind']+'-asset.json')).read_text('utf-8'));inspection=json.loads(get('/map-images/'+case['imageId']));assert inspection['asset']==asset
  png=base64.b64decode(inspection['imageUrl'].split(',')[1]);assert hashlib.sha256(png).hexdigest()==case['sha256'] and png==(out/(case['kind']+'-image.png')).read_bytes()
  package=get('/map-images/'+case['imageId']+'/export');assert package==(out/(case['kind']+'.zip')).read_bytes();checked.append(dict(imageId=case['imageId'],metadataExact=True,pngExact=True,exportExact=True))
 inspection=json.loads(get('/map-images/'+ui['asset']['id']));assert inspection['asset']==ui['asset']
 png=base64.b64decode(inspection['imageUrl'].split(',')[1]);assert hashlib.sha256(png).hexdigest()==ui['asset']['sha256']
 checked.append(dict(imageId=ui['asset']['id'],metadataExact=True,pngExact=True))
 # A rejecting proxy proves inspect/export have no dependency on the service.
 # This is isolated verification storage; it is restored before returning.
 def post(path,payload):return json.load(urllib.request.urlopen(urllib.request.Request(a.server+path,data=json.dumps(payload).encode(),headers={'Content-Type':'application/json','X-GeoD-Client':'geod-global'}),timeout=20))
 original=json.loads(get('/proxy'));post('/proxy',{'mode':'custom','url':'http://127.0.0.1:9'})
 try:
  for c in before['cases']:assert json.loads(get('/map-images/'+c['imageId']))['asset']['sha256']==c['sha256'];assert get('/map-images/'+c['imageId']+'/export')==(out/(c['kind']+'.zip')).read_bytes()
 finally:post('/proxy',original)
 report={'passed':True,'mode':'Actual process restart; offline inspection/export from managed files','networkNeededForOfflineRead':False,'rejectingProxyOfflineReadPassed':True,'images':checked}
 (out/'restart.json').write_text(json.dumps(report,indent=2),encoding='utf-8');print(json.dumps({'passed':True,'images':len(checked)}))
if __name__=='__main__':main()
