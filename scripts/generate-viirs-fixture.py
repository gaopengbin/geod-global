"""Generate explicitly synthetic full-size VIIRS reader fixtures with h5py.

This is a QA-only tool. The desktop runtime does not invoke Python or h5py.
Requires h5py==3.15.1. No fixture is a NASA product or an authenticated download.
"""
import argparse
import gzip
import hashlib
import json
import math
import tempfile
import shutil
from pathlib import Path

import h5py
import numpy as np

EDGE = 1200
RADIUS = 6371007.181
FILL = -28672


def structure(h, v, product):
    size = math.pi * RADIUS / 18
    name = f"{product[:3]}_Grid_1km_L3_2d"
    fields = "\n".join(f'''OBJECT=DataField_{index}
DataFieldName="SurfReflect_M{band}"
DataType=DFNT_INT16
DimList=("YDim","XDim")
END_OBJECT=DataField_{index}''' for index, band in enumerate([5, 4, 3], 1))
    return name, f'''GROUP=SwathStructure
END_GROUP=SwathStructure
GROUP=GridStructure
GROUP=GRID_1
GridName="{name}"
XDim=1200
YDim=1200
UpperLeftPointMtrs=({(h-18)*size:.9f},{(9-v)*size:.9f})
LowerRightMtrs=({(h-17)*size:.9f},{(8-v)*size:.9f})
Projection=GCTP_SNSOID
ProjParams=(6371007.181,0,0,0,0,0,0,0,0,0,0,0,0)
SphereCode=-1
GridOrigin=HDFE_GD_UL
PixelRegistration=HDFE_CENTER
GROUP=DataField
{fields}
END_GROUP=DataField
END_GROUP=GRID_1
END_GROUP=GridStructure
GROUP=PointStructure
END_GROUP=PointStructure
END
'''


def generate(output):
    output.mkdir(parents=True, exist_ok=True)
    fixtures = []
    negatives = []
    for product, endian, compression, libver in [
        ("VNP09A1", "<", True, "earliest"),
        ("VJ109A1", ">", True, "latest"),
        ("VJ209A1", "<", False, "earliest"),
    ]:
        item = f"{product}.A2025177.h08v05.002.2025333224010"
        grid, odl = structure(8, 5, product)
        expected = []
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "synthetic.h5"
            with h5py.File(path, "w", libver=libver) as file:
                attrs = file.create_group("HDFEOS/ADDITIONAL/FILE_ATTRIBUTES").attrs
                for name, value in {
                    "LocalGranuleID": f"{item}.h5", "ShortName": product,
                    "RangeBeginningDate": "2025-06-26", "RangeEndingDate": "2025-07-03",
                    "RangeBeginningTime": "00:00:00.000", "RangeEndingTime": "23:59:59.000",
                    "VersionID": "002", "SensorShortname": "VIIRS",
                    "HorizontalTileNumber": "08", "VerticalTileNumber": "05",
                    "FixtureProvenance": "SYNTHETIC: generated with h5py; not NASA data",
                }.items():
                    attrs[name] = np.bytes_(value)
                file.create_dataset("HDFEOS INFORMATION/StructMetadata.0", data=np.bytes_(odl), track_times=False)
                data_fields = file.create_group(f"HDFEOS/GRIDS/{grid}/Data Fields")
                for index, (key, band) in enumerate([("red", 5), ("green", 4), ("blue", 3)]):
                    rows, cols = np.indices((EDGE, EDGE), dtype=np.int32)
                    values = ((cols + rows * 3 + index * 137) % 16101 - 100).astype(f"{endian}i2")
                    values[(rows * EDGE + cols) % 10007 == 0] = FILL
                    # Preserve diagnostic out-of-range DN rather than silently masking it.
                    values[-1, -1] = 16001
                    options = dict(chunks=(137, 300), compression="gzip", compression_opts=4, shuffle=True, fletcher32=True) if compression else {}
                    dataset = data_fields.create_dataset(f"SurfReflect_M{band}", data=values, track_times=False, **options)
                    dataset.attrs["_FillValue"] = np.int16(FILL)
                    dataset.attrs["valid_range"] = np.array([-100, 16000], dtype="<i2")
                    dataset.attrs["scale_factor"] = np.float64(0.0001)
                    dataset.attrs["add_offset"] = np.float64(0)
                    dataset.attrs["units"] = np.bytes_("reflectance")
                    canonical = values.astype("<i2").tobytes(order="C")
                    nonfill = values[values != FILL]
                    expected.append(dict(band=key, samplesSha256=hashlib.sha256(canonical).hexdigest(),
                        noDataCount=int(np.count_nonzero(values == FILL)),
                        outsideValidRangeCount=int(np.count_nonzero((values != FILL) & ((values < -100) | (values > 16000)))),
                        minimum=int(nonfill.min()), maximum=int(nonfill.max())))
                chunk_offset = int(data_fields["SurfReflect_M5"].id.get_chunk_info_by_coord((0, 0)).byte_offset) if compression else None
            original = path.read_bytes()
            if product == "VNP09A1":
                for fault in ["wrong-calibration", "wrong-version", "wrong-grid", "wrong-type", "oversized-shape", "external-band"]:
                    changed = Path(temporary) / f"{fault}.h5"
                    shutil.copyfile(path, changed)
                    with h5py.File(changed, "r+") as file:
                        dataset_path = f"HDFEOS/GRIDS/{grid}/Data Fields/SurfReflect_M5"
                        if fault == "wrong-calibration":
                            file[dataset_path].attrs.modify("scale_factor", np.float64(0.001))
                        elif fault == "wrong-version":
                            file["HDFEOS/ADDITIONAL/FILE_ATTRIBUTES"].attrs.modify("VersionID", np.bytes_("001"))
                        elif fault == "wrong-grid":
                            file["HDFEOS INFORMATION/StructMetadata.0"][()] = np.bytes_(odl.replace("XDim=1200", "XDim=1201"))
                        else:
                            del file[dataset_path]
                            if fault == "external-band":
                                file[dataset_path] = h5py.ExternalLink("must-not-be-opened.h5", "/private")
                            else:
                                file.create_dataset(dataset_path, shape=(1200, 1200) if fault == "wrong-type" else (1200, 50000),
                                    dtype="<u2" if fault == "wrong-type" else "<i2", chunks=(100,100), track_times=False)
                    filename = f"negative-{fault}.h5.gz"
                    (output / filename).write_bytes(gzip.compress(changed.read_bytes(), mtime=0))
                    negatives.append(dict(file=filename, itemId=item, fault=fault))
                damaged = bytearray(original)
                damaged[chunk_offset + 3] ^= 1
                filename = "negative-damaged-chunk.h5.gz"
                (output / filename).write_bytes(gzip.compress(damaged, mtime=0))
                negatives.append(dict(file=filename, itemId=item, fault="damaged-chunk"))
        filename = f"synthetic-{product.lower()}.h5.gz"
        (output / filename).write_bytes(gzip.compress(original, mtime=0))
        fixtures.append(dict(file=filename, itemId=item, byteCount=len(original), sourceSha256=hashlib.sha256(original).hexdigest(),
            endian=endian, compressed=compression, libver=libver, bands=expected))
    (output / "expected.json").write_text(json.dumps(dict(provenance="Synthetic full 1200 x 1200 grids generated independently with h5py 3.15.1; not a real NASA download", fixtures=fixtures, negatives=negatives), indent=2) + "\n", encoding="utf-8")
    print(json.dumps(dict(fixtures=len(fixtures), output=str(output.resolve()), compressedBytes=sum(p.stat().st_size for p in output.glob("*.gz")))))


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, default=Path("crates/geod-runtime/fixtures/viirs"))
    generate(parser.parse_args().output)
