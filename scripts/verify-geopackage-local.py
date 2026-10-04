"""Independent full GeoPackage conversion check. QA-only GDAL/PROJ dependencies.

Compare every source feature, geometry coordinate, M value and SQLite attribute;
the exported original must match the entire source database byte for byte.
"""
import argparse
import base64
import hashlib
import json
import math
import sqlite3
from pathlib import Path
from osgeo import gdal, ogr
from pyproj import Transformer
from pyproj.enums import TransformDirection

gdal.UseExceptions()
SAFE = 9007199254740991
NAMES = {1:'Point',2:'LineString',3:'Polygon',4:'MultiPoint',5:'MultiLineString',6:'MultiPolygon',7:'GeometryCollection'}

def digest(b):
    return hashlib.sha256(b).hexdigest()

def quoted(s):
    return '"'+s.replace('"','""')+'"'

def geometry(g, transform):
    if g is None:
        return None, None
    kind = ogr.GT_Flatten(g.GetGeometryType())
    z, m = bool(ogr.GT_HasZ(g.GetGeometryType())), bool(ogr.GT_HasM(g.GetGeometryType()))
    def position(i):
        x,y = transform.transform(g.GetX(i),g.GetY(i),errcheck=True)
        p = [x,y]
        if z:p.append(g.GetZ(i))
        value = g.GetM(i) if m else None
        if value is not None and math.isnan(value):value=None
        return p,value
    if kind == 1:
        coordinates, measures = ([],[]) if g.IsEmpty() else position(0)
    elif kind == 2:
        pairs=[position(i) for i in range(g.GetPointCount())]
        coordinates=[p for p,v in pairs];measures=[v for p,v in pairs]
    elif kind == 3:
        coordinates,measures=[],[]
        for i in range(g.GetGeometryCount()):
            ring=g.GetGeometryRef(i)
            points=[];values=[]
            for j in range(ring.GetPointCount()):
                x,y=transform.transform(ring.GetX(j),ring.GetY(j),errcheck=True)
                points.append([x,y]+([ring.GetZ(j)] if z else []))
                value=ring.GetM(j) if m else None
                values.append(None if value is not None and math.isnan(value) else value)
            coordinates.append(points);measures.append(values)
    else:
        children=[geometry(g.GetGeometryRef(i),transform) for i in range(g.GetGeometryCount())]
        measures=[v for c,v in children]
        if kind==7:
            return {'type':'GeometryCollection','geometries':[c for c,v in children]},measures if m else None
        coordinates=[c['coordinates'] for c,v in children]
        if any(not c for c in coordinates):
            return {'type':'GeometryCollection','geodOriginalGeometryType':NAMES[kind],'geometries':[c for c,v in children]},measures if m else None
    return {'type':NAMES[kind],'coordinates':coordinates},measures if m else None

def compare(a,b,stats,where):
    if isinstance(a,dict):
        assert isinstance(b,dict) and set(a)==set(b),(where,a,b)
        for k in a:compare(a[k],b[k],stats,where+'.'+k)
    elif isinstance(a,list):
        assert isinstance(b,list) and len(a)==len(b),(where,a,b)
        for i,(x,y) in enumerate(zip(a,b)):compare(x,y,stats,where+f'[{i}]')
    elif isinstance(a,(int,float)) and not isinstance(a,bool):
        assert isinstance(b,(int,float)) and not isinstance(b,bool),(where,a,b)
        delta=abs(a-b);stats['numericOrdinates']+=1;stats['maximumCoordinateDifference']=max(stats['maximumCoordinateDifference'],delta)
        assert delta<=1e-8,(where,a,b,delta)
    else:
        assert a==b,(where,a,b)

class RecordedOperation:
    """Verify the declared operation, rather than PROJ's dynamic regional choice."""
    def __init__(self,wkt,layer):
        self.operation=Transformer.from_pipeline('EPSG:'+str(layer['coordinateOperationId']))
        assert self.operation.description==layer['coordinateOperation'],(self.operation.description,layer['coordinateOperation'])
        forward=layer['coordinateOperationDirection']=='forward'
        source=self.operation.source_crs if forward else self.operation.target_crs
        target=self.operation.target_crs if forward else self.operation.source_crs
        self.base=Transformer.from_crs(wkt,source,always_xy=True)
        self.in_yx=source.axis_info[0].direction=='north';self.out_yx=target.axis_info[0].direction=='north'
        self.direction=TransformDirection.FORWARD if forward else TransformDirection.INVERSE
        assert self.operation.accuracy==layer['coordinateAccuracyMeters']
        assert list(self.operation.area_of_use.bounds)==layer['coordinateOperationArea']
    def transform(self,x,y,errcheck=True):
        x,y=self.base.transform(x,y,errcheck=errcheck)
        a,b=self.operation.transform(y if self.in_yx else x,x if self.in_yx else y,direction=self.direction,errcheck=errcheck)
        return (b,a) if self.out_yx else (a,b)

def difference(a,b):
    if isinstance(a,dict):return max((difference(a[k],b[k]) for k in a),default=0.)
    if isinstance(a,list):return max((difference(x,y) for x,y in zip(a,b)),default=0.)
    return abs(a-b) if isinstance(a,(int,float)) and not isinstance(a,bool) else 0.

def verify(source,export,original,registry,identity):
    raw=source.read_bytes();data=json.loads(export.read_text(encoding='utf-8'))
    record=json.loads(registry.read_text(encoding='utf-8'))[identity]['asset']
    p=data['geodGeoPackage'];assert p==record['geoPackage']
    assert record['sourceSha256']==digest(raw) and record['bytes']==len(raw)
    assert original.read_bytes()==raw
    db=sqlite3.connect('file:'+source.resolve().as_posix()+'?mode=ro',uri=True)
    ds=ogr.Open(str(source));assert ds is not None
    by_layer={l['table']:[] for l in p['layers']}
    for f in data['features']:by_layer[f['geodLayer']].append(f)
    stats={'features':0,'properties':0,'numericOrdinates':0,'maximumCoordinateDifference':0.0,'maximumDifferenceFromAutomaticRegionalPROJ':0.0,'sourceSha256':digest(raw),'sourceBytes':len(raw),'layers':[],'originalExportIdentical':True}
    for layer in p['layers']:
        table=layer['table'];columns=db.execute('PRAGMA table_xinfo('+quoted(table)+')').fetchall()
        assert [(r[1],r[2],r[3]==0 and r[5]==0,r[5]==1) for r in columns]==[(f['name'],f['fieldType'],f['nullable'],f['primaryKey']) for f in layer['fields']]
        metadata=db.execute('SELECT column_name,geometry_type_name,srs_id,z,m FROM gpkg_geometry_columns WHERE table_name=?',(table,)).fetchone()
        assert list(metadata)==[layer['geometryColumn'],layer['geometryType'],layer['srsId'],layer['z'],layer['m']]
        definitions=db.execute('SELECT organization,organization_coordsys_id,definition FROM gpkg_spatial_ref_sys WHERE srs_id=?',(layer['srsId'],)).fetchone()
        assert list(definitions)==[layer['organization'],layer['organizationCoordsysId'],layer['definition']]
        ogr_layer=ds.GetLayerByName(table);assert ogr_layer.GetFeatureCount()==layer['featureCount']
        default_transform=Transformer.from_crs(ogr_layer.GetSpatialRef().ExportToWkt(),'EPSG:4326',always_xy=True)
        transform=RecordedOperation(ogr_layer.GetSpatialRef().ExportToWkt(),layer) if layer['coordinateOperationId'] is not None else default_transform
        key=next(f['name'] for f in layer['fields'] if f['primaryKey']);rows=db.execute('SELECT '+','.join(quoted(f['name']) for f in layer['fields'])+' FROM '+quoted(table)+' ORDER BY '+quoted(key)).fetchall()
        assert len(rows)==len(by_layer[table])==layer['featureCount']
        for row,feature in zip(rows,by_layer[table]):
            properties={};fid=None
            for field,value in zip(layer['fields'],row):
                if field['jsonEncoding']=='geometry':continue
                if field['primaryKey']:fid=value
                if isinstance(value,bytes):value=base64.b64encode(value).decode('ascii')
                elif field['jsonEncoding']=='boolean' and value is not None:value=bool(value)
                elif isinstance(value,int) and abs(value)>SAFE:value=str(value)
                properties[field['name']]=value
            assert feature['id']==(str(fid) if abs(fid)>SAFE else fid)
            assert properties==feature['properties'],(table,fid,properties,feature['properties'])
            ogr_feature=ogr_layer.GetFeature(fid);expected,measures=geometry(ogr_feature.GetGeometryRef(),transform)
            compare(expected,feature['geometry'],stats,f'{table}/{fid}.geometry')
            automatic,_=geometry(ogr_feature.GetGeometryRef(),default_transform)
            stats['maximumDifferenceFromAutomaticRegionalPROJ']=max(stats['maximumDifferenceFromAutomaticRegionalPROJ'],difference(automatic,feature['geometry']))
            if measures is not None:compare(measures,feature['geodMeasures'],stats,f'{table}/{fid}.M')
            else:assert 'geodMeasures' not in feature
            stats['features']+=1;stats['properties']+=len(properties)
        stats['layers'].append({'table':table,'features':len(rows),'sourceCrs':ogr_layer.GetSpatialRef().GetAuthorityCode(None),'coordinateOperation':layer['coordinateOperation'],'coordinateOperationId':layer['coordinateOperationId'],'accuracyMetersWithinDeclaredArea':layer['coordinateAccuracyMeters'],'coordinatesOutsideDeclaredArea':layer['coordinatesOutsideOperationArea']})
    assert stats['features']==record['featureCount']
    assert stats['features']==len(data['features'])
    assert p['horizontalCrs']=='EPSG:4326' and p['verticalConversion']=='source-z-retained; no-vertical-transformation'
    assert p['measureEncoding']=='geodMeasures; source-values; NaN-as-null'
    stats['independentReader']='GDAL '+gdal.VersionInfo('--version')+'; pyproj PROJ transformations; SQLite raw attributes'
    return stats

if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    for name in ['source','export','original-export','registry','report']:parser.add_argument('--'+name,type=Path,required=True)
    parser.add_argument('--id',required=True);args=parser.parse_args()
    result=verify(args.source,args.export,args.original_export,args.registry,args.id)
    args.report.parent.mkdir(parents=True,exist_ok=True);args.report.write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
    print(json.dumps(result,ensure_ascii=False))
