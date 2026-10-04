"""Independent PyShp + GDAL CRS controls. Never overwrite an existing fixture.

GDAL and PyShp are QA tools, not application dependencies. Exact numeric DBF
lexemes and the deletion flag are patched explicitly after the producer writes
the tables, so QA exercises details that an OGR GeoJSON export would discard.
"""
import argparse
import datetime
import hashlib
import json
import struct
import tempfile
import zipfile
from pathlib import Path
import shapefile
from osgeo import gdal, osr
osr.UseExceptions()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    output = args.out.resolve()
    if output.exists() or output.suffix.lower() != '.zip':
        parser.error('--out must be a new ZIP file')
    output.parent.mkdir(parents=True, exist_ok=True)
    definitions = {}
    with tempfile.TemporaryDirectory(prefix='geod-shp-control-') as directory:
        root = Path(directory)
        for shape_type in (1, 3, 5, 8, 11, 13, 15, 18, 21, 23, 25, 28):
            name = f'type_{shape_type:02d}'
            target = root / name
            epsg = 32633 if shape_type in (5, 15, 25) else 3857 if shape_type == 1 else 4326
            encoding = 'gbk' if shape_type == 21 else 'utf-8'
            writer = shapefile.Writer(str(target), shapeType=shape_type, encoding=encoding)
            writer.field('name', 'C', 48)
            writer.field('large', 'N', 20, 0)
            writer.field('precise', 'N', 30, 18)
            writer.field('active', 'L')
            writer.field('date', 'D')
            writer.field('empty', 'N', 12, 2)
            base = shape_type if shape_type < 10 else shape_type % 10

            def row():
                writer.record('  中文 München' if encoding == 'utf-8' else '  中文属性',
                              9007199254740993, 1.25, True, datetime.date(2026, 10, 3), None)

            if base == 1:
                if shape_type == 1:
                    writer.point(1335833.8895192828, 6106854.834885074)
                elif shape_type == 11:
                    writer.pointz(12, 48, 18.25, 7.5)
                else:
                    writer.pointm(12, 48, 5.25)
                row()
                if shape_type == 11:
                    writer.pointz(13, 49, -2.5, None)
                    row()
                    writer.null()
                    row()
            elif base == 3:
                parts = [[[12, 48], [13, 49], [14, 50]], [[15, 47], [16, 48]]]
                if shape_type == 13:
                    writer.linez([[[x, y, 10 + i, 2.5 + i] for i, (x, y) in enumerate(p)] for p in parts])
                elif shape_type == 23:
                    writer.linem([[[x, y, None if i == 1 else i + .5] for i, (x, y) in enumerate(p)] for p in parts])
                else:
                    writer.line(parts)
                row()
            elif base == 5:
                shell = [(0, 0), (0, 1000), (1000, 1000), (1000, 0), (0, 0)]
                hole = [(200, 200), (400, 200), (400, 400), (200, 400), (200, 200)]
                island = [(2000, 0), (2000, 1000), (3000, 1000), (3000, 0), (2000, 0)]
                parts = [[[500000 + x, 4649776.22482 + y] for x, y in p] for p in (hole, island, shell)]
                if shape_type == 15:
                    writer.polyz([[[x, y, 20 + i, 1.5 + i] for i, (x, y) in enumerate(p)] for p in parts])
                elif shape_type == 25:
                    writer.polym([[[x, y, None if i == 1 else i + .5] for i, (x, y) in enumerate(p)] for p in parts])
                else:
                    writer.poly(parts)
                row()
            else:
                points = [[12, 48], [13, 49]]
                if shape_type == 18:
                    writer.multipointz([[x, y, i + .25, i + .5] for i, (x, y) in enumerate(points)])
                elif shape_type == 28:
                    writer.multipointm([[x, y, None if i else .5] for i, (x, y) in enumerate(points)])
                else:
                    writer.multipoint(points)
                row()
            writer.close()
            dbf = target.with_suffix('.dbf')
            data = bytearray(dbf.read_bytes())
            data[1:4] = bytes((126, 10, 3))  # freeze producer header date for reproducibility
            count, header, length = struct.unpack_from('<IHH', data, 4)
            for i in range(count):
                start = header + i * length + 1 + 48 + 20
                data[start:start + 30] = b'1234567890.123456789012345678'.rjust(30)
            if shape_type == 11:
                data[header + length] = ord('*')
            dbf.write_bytes(data)
            srs = osr.SpatialReference()
            srs.ImportFromEPSG(epsg)
            srs.MorphToESRI()
            prj = srs.ExportToWkt()
            target.with_suffix('.prj').write_text(prj, encoding='utf-8')
            target.with_suffix('.cpg').write_text('GBK' if encoding == 'gbk' else 'UTF-8', encoding='ascii')
            definitions[name] = {'epsg': epsg, 'shapeType': shape_type, 'records': count,
                                 'deleted': 1 if shape_type == 11 else 0, 'encoding': encoding}
        (root / 'README.txt').write_text('GeoD independent QA controls; no third-party geographic data.\n', encoding='utf-8')
        with zipfile.ZipFile(output, 'w', zipfile.ZIP_DEFLATED) as archive:
            for path in sorted(root.iterdir()):
                info = zipfile.ZipInfo(path.name, date_time=(2026, 10, 3, 0, 0, 0))
                info.compress_type = zipfile.ZIP_DEFLATED
                info.external_attr = 0o100644 << 16
                archive.writestr(info, path.read_bytes())
    receipt = {'schemaVersion': 'geod-shapefile-fixture/v1', 'file': output.name,
               'bytes': output.stat().st_size, 'sha256': hashlib.sha256(output.read_bytes()).hexdigest(),
               'producer': {'pyshp': shapefile.__version__, 'gdal': gdal.VersionInfo('--version')},
               'license': 'LicenseRef-Proprietary', 'origin': 'Own synthetic controls, generated by this script',
               'notes': ['Original numeric lexeme and DBF deletion flag explicitly patched after PyShp writes.',
                         'Polygon rings intentionally out of order; hole, island and shell remain distinct.',
                         'All 12 supported types; WGS84, ESRI Web Mercator and UTM 33N, Z/M and NoData.'],
               'layers': definitions}
    output.with_suffix('.SOURCE.json').write_text(json.dumps(receipt, ensure_ascii=False, indent=2)+'\n', encoding='utf-8')
    print(json.dumps(receipt, ensure_ascii=False))


if __name__ == '__main__':
    main()
