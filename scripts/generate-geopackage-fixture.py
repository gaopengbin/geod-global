"""Generate independent GeoPackage controls using OGR; QA dependency only.

The frozen fixture has its own SHA-256 receipt. A regeneration does not promise
identical SQLite layout or GDAL timestamps. Existing files are never overwritten.
"""
import argparse
import hashlib
import json
import sqlite3
from pathlib import Path
from osgeo import gdal, ogr, osr


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    output = args.out.resolve()
    if output.suffix.lower() != '.gpkg' or output.exists():
        parser.error('--out must be a new .gpkg file')
    output.parent.mkdir(parents=True, exist_ok=True)
    gdal.UseExceptions()
    dataset = ogr.GetDriverByName('GPKG').CreateDataSource(str(output), options=['VERSION=1.4'])

    def layer(name, epsg, kind):
        srs = osr.SpatialReference()
        srs.ImportFromEPSG(epsg)
        srs.SetAxisMappingStrategy(osr.OAMS_TRADITIONAL_GIS_ORDER)
        return dataset.CreateLayer(name, srs, kind, options=['SPATIAL_INDEX=YES'])

    def fields(target):
        for name, kind in [('name', ogr.OFTString), ('large', ogr.OFTInteger64),
                           ('number', ogr.OFTReal), ('active', ogr.OFTInteger),
                           ('payload', ogr.OFTBinary), ('date', ogr.OFTDate),
                           ('datetime', ogr.OFTDateTime)]:
            field = ogr.FieldDefn(name, kind)
            if name == 'active':
                field.SetSubType(ogr.OFSTBoolean)
            assert target.CreateField(field) == ogr.OGRERR_NONE

    def feature(target, fid, wkt):
        value = ogr.Feature(target.GetLayerDefn())
        value.SetFID(fid)
        if target.GetLayerDefn().GetFieldCount():
            value.SetField('name', '独立 GDAL 样本')
            value.SetField('large', 9007199254740993)
            value.SetField('number', 1.25)
            value.SetField('active', 1)
            value.SetFieldBinaryFromHexString('payload', '00ff')
            value.SetField('date', '2026/10/03')
            value.SetField('datetime', '2026/10/03 10:30:00+00')
        if wkt is not None:
            value.SetGeometry(ogr.CreateGeometryFromWkt(wkt))
        assert target.CreateFeature(value) == ogr.OGRERR_NONE

    target = layer('all', 4326, ogr.wkbPointZM)
    fields(target)
    feature(target, 9007199254740993, 'POINT ZM (12 48 18.25 7.5)')
    feature(target, 2, 'POINT ZM (13 49 10 2.5)')
    feature(target, 3, None)
    feature(target, 4, 'POINT ZM EMPTY')
    target = layer('routes_m', 4326, ogr.wkbLineStringM)
    fields(target)
    feature(target, 2, 'LINESTRING M (12 48 1,13 49 2,14 50 3)')
    target = layer('web_mercator', 3857, ogr.wkbPoint)
    feature(target, 1, 'POINT (1335833.8895192828 6106854.834885074)')
    target = layer('utm33', 32633, ogr.wkbPolygon)
    feature(target, 1, 'POLYGON ((300000 5300000,310000 5300000,310000 5310000,300000 5300000),(302000 5301000,303000 5301000,303000 5302000,302000 5301000))')
    target = layer('collection', 4326, ogr.wkbGeometryCollection)
    feature(target, 1, 'GEOMETRYCOLLECTION (POINT (1 2),LINESTRING (1 2,2 3))')
    target = None
    dataset = None
    with sqlite3.connect(output) as db:
        db.execute('CREATE TABLE source_notes (id INTEGER PRIMARY KEY, note TEXT)')
        db.execute("INSERT INTO source_notes VALUES (1,'retained original content')")
        db.execute("INSERT INTO gpkg_contents (table_name,data_type,identifier,description) VALUES ('source_notes','attributes','Notes','nonspatial contents')")
    raw = output.read_bytes()
    print(json.dumps({'file': str(output), 'bytes': len(raw), 'sha256': hashlib.sha256(raw).hexdigest(),
                      'gdalVersion': gdal.VersionInfo('--version'),
                      'origin': 'Independently generated test controls; not a downloaded production dataset'}))


if __name__ == '__main__':
    main()
