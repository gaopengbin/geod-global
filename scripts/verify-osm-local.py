"""Independent byte/identity/attribute/geometry comparison with Pyosmium 4.3.1.

This verifier never imports GeoD's OSM decoder or geometry converter. JSON output
must contain all original objects, even untagged dependency nodes. Polygon ring
orientation and starting vertex are immaterial; every ordinate is compared.
"""
from pathlib import Path
import argparse,json,math,hashlib,importlib.metadata
import osmium

class Reader(osmium.SimpleHandler):
    def __init__(self):
        super().__init__();self.objects={}
    def remember(self,kind,o,**extra):
        key=f'{kind}/{o.id}';assert key not in self.objects,'Duplicate source identity'
        self.objects[key]={'id':o.id,'kind':kind,'tags':dict(o.tags),'metadata':{'version':o.version,'changeset':o.changeset,'uid':o.uid,'user':o.user,'visible':o.visible,'timestamp':o.timestamp.isoformat().replace('+00:00','Z')},**extra}
    def node(self,o):self.remember('node',o,point=[o.location.lon,o.location.lat])
    def way(self,o):self.remember('way',o,nodes=[n.ref for n in o.nodes],points=[[n.location.lon,n.location.lat] if n.location.valid() else None for n in o.nodes])
    def relation(self,o):self.remember('relation',o,members=[{'type':{'n':'node','w':'way','r':'relation'}[m.type],'ref':m.ref,'role':m.role} for m in o.members])
class Areas(osmium.SimpleHandler):
    def __init__(self):super().__init__();self.areas={};self.factory=osmium.geom.GeoJSONFactory()
    def area(self,o):self.areas[('way' if o.from_way() else 'relation')+'/'+str(o.orig_id())]=json.loads(self.factory.create_multipolygon(o))

def rings(g):
    polygons=[g['coordinates']] if g['type']=='Polygon' else g['coordinates']
    def ring(points):
        p=[tuple(round(float(n),7) for n in pt) for pt in points[:-1]]
        assert len(p)>=3 and all(math.isfinite(n) for pt in p for n in pt)
        return min(tuple(p[i:]+p[:i]) for p in (p,list(reversed(p))) for i in range(len(p)))
    return sorted((ring(p[0]),tuple(sorted(ring(r) for r in p[1:]))) for p in polygons)

def verify(source,converted,original):
    source,converted,original=map(Path,(source,converted,original))
    raw=source.read_bytes();assert original.read_bytes()==raw,'Original export differs byte for byte'
    data=json.loads(converted.read_text(encoding='utf-8'));reader=Reader();reader.apply_file(str(source),locations=False)
    for o in reader.objects.values():
        if o['kind']=='way':o['points']=[p if p is not None else reader.objects['node/'+str(n)]['point'] for n,p in zip(o['nodes'],o['points'])]
    areas=Areas()
    if any(f['properties']['osm_type']=='relation' and f['geometry']['type'] in ('Polygon','MultiPolygon') for f in data['features']):areas.apply_file(str(source),locations=True)
    assert data['type']=='FeatureCollection' and len(data['features'])==len(reader.objects)
    counts={'node':0,'way':0,'relation':0};ordinates=0;attrs=0;max_difference=0.;polygon_count=0
    def geometry(actual,expected):
        nonlocal ordinates,max_difference,polygon_count
        if actual['type'] in ('Polygon','MultiPolygon') and expected['type'] in ('Polygon','MultiPolygon'):
            assert rings(actual)==rings(expected);polygon_count+=1;return
        assert actual['type']==expected['type'],(actual,expected)
        if actual['type']=='GeometryCollection':
            assert len(actual['geometries'])==len(expected['geometries'])
            for a,b in zip(actual['geometries'],expected['geometries']):geometry(a,b)
        else:
            def coords(a,b):
                nonlocal ordinates,max_difference
                if isinstance(a,list):
                    assert len(a)==len(b)
                    for a,b in zip(a,b):coords(a,b)
                else:
                    d=abs(a-b);max_difference=max(max_difference,d);ordinates+=1;assert d<=1e-7,(a,b,d)
            coords(actual['coordinates'],expected['coordinates'])
    def expected(o,as_member=False):
        if o['kind']=='node':return {'type':'Point','coordinates':o['point']}
        if o['kind']=='way':return {'type':'LineString','coordinates':o['points']}
        if o['tags'].get('type') in ('multipolygon','boundary'):return areas.areas['relation/'+str(o['id'])]
        return {'type':'GeometryCollection','geometries':[expected(reader.objects[m['type']+'/'+str(m['ref'])],True) for m in o['members']]}
    ids=[]
    for f in data['features']:
        o=reader.objects[f['id']];ids.append(f['id']);counts[o['kind']]+=1;p=f['properties']
        assert p['osm_type']==o['kind'] and p['osm_id']==o['id'] and p['tags']==o['tags'];attrs+=len(o['tags'])
        if o['kind']=='way':assert p['osm_nodes']==o['nodes'];attrs+=len(o['nodes'])
        if o['kind']=='relation':assert p['osm_members']==o['members'];attrs+=len(o['members'])
        for k,v in p.get('osm_metadata',{}).items():
            if k=='timestamp':assert __import__('datetime').datetime.fromisoformat(v.replace('Z','+00:00'))==__import__('datetime').datetime.fromisoformat(o['metadata'][k].replace('Z','+00:00'))
            else:assert v==o['metadata'][k],(f['id'],k,v,o['metadata'][k])
            attrs+=1
        if f['geometry']['type'] in ('Polygon','MultiPolygon'):
            if o['kind']=='way':independent={'type':'Polygon','coordinates':[o['points']]}
            else:
                assert f['id'] in areas.areas,(f['id'],'no independent assembled area')
                independent=areas.areas[f['id']]
            assert rings(f['geometry'])==rings(independent),f['id'];polygon_count+=1
        else:geometry(f['geometry'],expected(o))
    assert len(set(ids))==len(reader.objects)
    assert data['geodLocalOsm']['objectCounts']==counts
    return {'source':str(source),'sourceSHA256':hashlib.sha256(raw).hexdigest(),'originalBytes':len(raw),'originalExportIdentical':True,'independentReader':'Pyosmium '+importlib.metadata.version('osmium'),'objects':counts,'attributesAndReferencesCompared':attrs,'nonPolygonOrdinatesCompared':ordinates,'polygonTopologyAndAllRoundedOrdinatesCompared':polygon_count,'coordinateToleranceDegrees':1e-7,'maximumNonPolygonDifferenceDegrees':max_difference}

if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('source');p.add_argument('converted');p.add_argument('original');p.add_argument('--report');a=p.parse_args();r=verify(a.source,a.converted,a.original)
    if a.report:Path(a.report).write_text(json.dumps(r,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
    print(json.dumps(r,ensure_ascii=False))
