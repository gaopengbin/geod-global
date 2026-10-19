"""Build the finite package index from one complete, locally downloaded official CSV.

No network, row filtering or replacement data. Every publisher row must be valid.
"""
import csv
import hashlib
import io
import json
from pathlib import Path
import sys

SOURCE = "https://bfppub.blob.core.windows.net/%24web/2026-08-13/dataset-links.csv"
EXPECTED_SHA256 = "de61b569b0c23c364d61173632d8c5487ea30bf20912c55b9fa1ab4a89d01148"


def main():
    source, target = map(Path, sys.argv[1:])
    original = source.read_bytes()
    digest = hashlib.sha256(original).hexdigest()
    if digest != EXPECTED_SHA256:
        raise ValueError("The complete pinned publisher CSV hash does not match")
    records = list(csv.DictReader(io.StringIO(original.decode("utf-8"))))
    if len(records) != 30340:
        raise ValueError("The complete official 2026-08-13 index must contain 30340 rows")
    result = []
    seen = set()
    for row in records:
        country, quadkey, url = row["Location"], row["QuadKey"], row["Url"]
        prefix = f"https://bfppub.z5.web.core.windows.net/2026-08-13/global-buildings.geojsonl/RegionName={country}/quadkey={quadkey}/"
        if not country.isascii() or not country.replace("-", "").isalnum() or len(quadkey) != 9 or set(quadkey) - set("0123"):
            raise ValueError("Invalid country or quadkey in publisher CSV")
        if row["UploadDate"] != "2026-08-13" or not url.startswith(prefix):
            raise ValueError("URL/release differs from pinned official product")
        part = url[len(prefix):]
        if "/" in part or not part.startswith("part-") or not part.endswith(".c000.csv.gz"):
            raise ValueError("Invalid full original part filename")
        if (country, quadkey) in seen:
            raise ValueError("Duplicate publisher package identity")
        seen.add((country, quadkey))
        result.append([country, quadkey, part, row["Size"]])
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(json.dumps({"sourceSha256": digest, "rows": result}, separators=(",", ":")) + "\n", encoding="utf-8")
    print(json.dumps({"source": SOURCE, "sourceBytes": len(original), "sourceSha256": digest, "rows": len(result), "indexBytes": target.stat().st_size}))


if __name__ == "__main__":
    main()
