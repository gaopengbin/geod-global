"""Derive multilingual Agent aliases from the reviewed Natural Earth v5.1.1 ZIP.

Keeps the existing UI boundary/index files unchanged. Requires pyshp only at
development time. Coordinates remain in the separately verified UI index.
"""
import argparse
import hashlib
import io
import json
from pathlib import Path
import zipfile
import unicodedata
import shapefile

SOURCE_SHA = "efc59726337323058f9446210adc96673179cd344e053666ee3d28cb58ba2b05"
SOURCE = "https://naturalearth.s3.amazonaws.com/10m_cultural/ne_10m_admin_1_states_provinces.zip"
PREFIX = "ne_10m_admin_1_states_provinces."

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("zip", type=Path)
    args = parser.parse_args()
    raw = args.zip.read_bytes()
    assert hashlib.sha256(raw).hexdigest() == SOURCE_SHA, "Reviewed Natural Earth source hash required"
    aliases = {}
    with zipfile.ZipFile(io.BytesIO(raw)) as archive:
        reader = shapefile.Reader(shp=io.BytesIO(archive.read(PREFIX + "shp")),
                                  shx=io.BytesIO(archive.read(PREFIX + "shx")),
                                  dbf=io.BytesIO(archive.read(PREFIX + "dbf")), encoding="utf-8")
        for record in reader.iterRecords():
            p = record.as_dict()
            names = sorted({v.strip() for k, v in p.items()
                            if (k.startswith("name") or k == "iso_3166_2")
                            and isinstance(v, str) and v.strip() and v.strip() != "-99"
                            and len(v.strip()) <= 200 and not any(unicodedata.category(c) == "Cc" for c in v)})
            assert len(names) <= 40
            aliases[p["adm1_code"]] = names
    assert len(aliases) == 4596
    root = Path(__file__).resolve().parents[1]
    output = root / "prototype/public/basemaps/admin1-10m/agent-aliases.json"
    output.write_text(json.dumps({"source": SOURCE, "sourceSha256": SOURCE_SHA,
                                 "version": "5.1.1", "license": "Natural Earth public domain",
                                 "aliases": aliases}, ensure_ascii=False, separators=(",", ":")), encoding="utf-8")
    print(json.dumps({"units": len(aliases), "bytes": output.stat().st_size,
                      "sha256": hashlib.sha256(output.read_bytes()).hexdigest()}))

if __name__ == "__main__":
    main()
