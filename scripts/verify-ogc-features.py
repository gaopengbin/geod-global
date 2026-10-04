"""Real public-service acceptance; no credentials, mocks or desktop automation.

Run against an isolated GeoD loopback runtime. Refetches page receipt URLs and
independently compares original features with the saved native GeoJSON snapshot.
"""
import argparse
import hashlib
import json
from pathlib import Path
import urllib.request
from shapely.geometry import shape, box

parser=argparse.ArgumentParser()
parser.add_argument('--server',default='http://127.0.0.1:4359')
parser.add_argument('--out',default='.verification/ogc-real-evidence.json')
args=parser.parse_args()
def request(url,body=None):
    headers={'Accept':'application/json, application/geo+json'}
    if body is not None:
        headers.update({'Content-Type':'application/json','X-GeoD-Client':'geod-global'})
    req=urllib.request.Request(url,data=None if body is None else json.dumps(body).encode(),headers=headers)
    with urllib.request.urlopen(req,timeout=150) as response:
        raw=response.read(20*1024*1024+1)
        assert len(raw)<=20*1024*1024
        return json.loads(raw),raw
service,_=request(args.server+'/feature-services',{'name':'pygeoapi · Natural Earth','url':'https://demo.pygeoapi.io/stable'})
assert any(c['id']=='lakes' for c in service['collections'])
asset,_=request(args.server+'/feature-services/query',{'serviceId':service['id'],'collectionId':'lakes','bounds':[-180,-90,180,90],'pageSize':2})
inspection,_=request(args.server+'/vectors/'+asset['id'])
data=inspection['geojson'];source=asset['remoteSource']
assert source==data['geodSource']
assert source['selection']=='bbox-full-features'
assert source['requestedBounds']==[-180,-90,180,90]
assert len(source['pages'])>1
features=[];seen={};geometry_counts={};holes=0;expected_bounds=None
for receipt in source['pages']:
    page,raw=request(receipt['url'])
    # A service may refresh a response timestamp. Hashes pin the received bytes;
    # geometry and attributes must match independently refetched pages exactly.
    assert page['numberReturned']==receipt['returned']==len(page['features'])
    for feature in page['features']:
        key=json.dumps(feature.get('id'),sort_keys=True) if 'id' in feature else None
        if key is not None and key in seen:
            assert feature==seen[key]
            continue
        if key is not None:seen[key]=feature
        features.append(feature)
assert features==data['features']
assert len(features)==source['numberMatched']==source['featureCount']==asset['featureCount']
for f in features:
    geometry=shape(f['geometry'])
    assert geometry.is_valid and geometry.intersects(box(-180,-90,180,90))
    b=geometry.bounds
    expected_bounds=list(b) if expected_bounds is None else [min(expected_bounds[0],b[0]),min(expected_bounds[1],b[1]),max(expected_bounds[2],b[2]),max(expected_bounds[3],b[3])]
    kind=f['geometry']['type'];geometry_counts[kind]=geometry_counts.get(kind,0)+1
    parts=list(geometry.geoms) if kind=='MultiPolygon' else [geometry] if kind=='Polygon' else []
    holes+=sum(len(p.interiors) for p in parts)
assert expected_bounds==asset['bounds']
assert geometry_counts==asset['geometryCounts']
evidence={'verifiedAt':source['requestedAt'],'source':'OGC API Features / pygeoapi public demonstration service / Natural Earth Large Lakes',
          'serviceUrl':service['url'],'serviceId':service['id'],'assetId':asset['id'],'supportedCollections':len(service['collections']),
          'requestBounds':source['requestedBounds'],'pages':len(source['pages']),'features':len(features),'geometryCounts':geometry_counts,'holes':holes,
          'sourceSha256':asset['sourceSha256'],'geojsonSha256':asset['geojsonSha256'],
          'nativeProvenanceMatchesExport':True,'independentOriginalFeaturesAndAttributes':'exact equality with refetched source pages',
          'independentGeometryValidation':'Shapely/GEOS; valid geometry, bbox intersection, bounds and polygon holes',
          'datasetLicenseLinks':source['licenseLinks'],'limitations':['public developer demo availability is not guaranteed','complete matching bbox query only; full geometry, no implicit clip','request time is not an observation date','source page receipt hashes cover received bytes; a later refetch can have a changed response timestamp']}
Path(args.out).write_text(json.dumps(evidence,ensure_ascii=False,indent=2)+'\n',encoding='utf8')
print(json.dumps(evidence,ensure_ascii=False))
