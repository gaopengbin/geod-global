"""Build reproducible, lazily loaded Natural Earth Admin-1 browser assets.

Input: official ne_10m_admin_1_states_provinces.zip (v5.1.1). This is a
developer-only conversion; pyshp is never needed by the shipped application.
"""
import argparse
import hashlib
import io
import json
import math
import re
import zipfile
from collections import defaultdict
from pathlib import Path

import shapefile

SOURCE_SHA256 = "efc59726337323058f9446210adc96673179cd344e053666ee3d28cb58ba2b05"
SOURCE_URL = "https://naturalearth.s3.amazonaws.com/10m_cultural/ne_10m_admin_1_states_provinces.zip"
ROOT = Path(__file__).resolve().parents[1]
DESTINATION = ROOT / "prototype/public/basemaps/admin1-10m"
PREFIX = "ne_10m_admin_1_states_provinces."


def compact_json(value):
    return json.dumps(value, ensure_ascii=False, separators=(",", ":"), allow_nan=False).encode("utf-8")


def clip_limitation(geometry, bounds):
    """Mirror the v2 recipe's static geometry limits for early UI feedback."""
    if bounds[1] < -80 or bounds[3] > 84:
        return "polar"
    if bounds[2] - bounds[0] > 180:
        return "date-line"
    polygons = [geometry["coordinates"]] if geometry["type"] == "Polygon" else geometry["coordinates"]
    if len(polygons) > 500:
        return "complex"
    positions = 0
    for polygon in polygons:
        if len(polygon) > 1000:
            return "complex"
        for ring in polygon:
            positions += len(ring)
            if positions > 30000:
                return "complex"
            if any(abs(a[0] - b[0]) > 180 for a, b in zip(ring, ring[1:])):
                return "date-line"
    return ""


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("zip_path", type=Path, help="Official Natural Earth Admin-1 v5.1.1 zip")
    args = parser.parse_args()
    raw = args.zip_path.read_bytes()
    digest = hashlib.sha256(raw).hexdigest()
    if digest != SOURCE_SHA256:
        raise SystemExit(f"Unexpected Natural Earth source SHA-256: {digest}")
    with zipfile.ZipFile(io.BytesIO(raw)) as archive:
        if archive.read(PREFIX + "VERSION.txt").decode("ascii").strip() != "5.1.1":
            raise SystemExit("Expected Natural Earth Admin-1 version 5.1.1")
        reader = shapefile.Reader(
            shp=io.BytesIO(archive.read(PREFIX + "shp")),
            shx=io.BytesIO(archive.read(PREFIX + "shx")),
            dbf=io.BytesIO(archive.read(PREFIX + "dbf")),
            encoding="utf-8",
        )
        by_country = defaultdict(list)
        index = []
        for item in reader.iterShapeRecords():
            properties = item.record.as_dict()
            country = properties["adm0_a3"]
            code = properties["adm1_code"]
            if not re.fullmatch(r"[A-Z]{3}", country) or not code or item.shape.shapeType not in (5, 15, 25):
                raise SystemExit(f"Invalid Admin-1 record: {country}/{code}")
            geometry = item.shape.__geo_interface__
            if geometry["type"] not in ("Polygon", "MultiPolygon"):
                raise SystemExit(f"Unsupported geometry type for {code}: {geometry['type']}")
            name_en = properties.get("name_en") or properties.get("name") or code
            name_zh = properties.get("name_zh") or ""
            name_local = properties.get("name_local") or ""
            bounds = [math.floor(value * 1_000_000) / 1_000_000 if index < 2
                      else math.ceil(value * 1_000_000) / 1_000_000
                      for index, value in enumerate(item.shape.bbox)]
            limitation = clip_limitation(geometry, bounds)
            feature_properties = {
                "adm1_code": code, "adm0_a3": country,
                "name_en": name_en, "name_zh": name_zh,
                "name_local": name_local,
                "clip_limitation": limitation,
                "iso_3166_2": properties.get("iso_3166_2") or "",
            }
            index.append({"code": code, "parentCode": country, "nameEn": name_en,
                          "nameZh": name_zh, "nameLocal": name_local, "bounds": bounds,
                          "clipLimitation": limitation})
            by_country[country].append({"type": "Feature", "properties": feature_properties, "geometry": geometry})

    DESTINATION.mkdir(parents=True, exist_ok=True)
    limitations = {reason: sum(entry["clipLimitation"] == reason for entry in index)
                   for reason in ("polar", "date-line", "complex")}
    manifest = {"source": SOURCE_URL, "sourceSha256": SOURCE_SHA256, "version": "5.1.1",
                "license": "Natural Earth public domain", "featureCount": len(index),
                "countryCount": len(by_country), "clipLimitations": limitations, "countries": {}}
    for country, features in sorted(by_country.items()):
        features.sort(key=lambda feature: feature["properties"]["adm1_code"])
        payload = compact_json({"type": "FeatureCollection", "features": features})
        filename = f"{country}.geojson"
        (DESTINATION / filename).write_bytes(payload)
        manifest["countries"][country] = {"file": filename, "features": len(features),
                                            "bytes": len(payload), "sha256": hashlib.sha256(payload).hexdigest()}
    index.sort(key=lambda entry: entry["code"])
    index_payload = compact_json({"source": "Natural Earth 1:10m Admin-1 v5.1.1", "areas": index})
    (DESTINATION / "index.json").write_bytes(index_payload)
    manifest["indexSha256"] = hashlib.sha256(index_payload).hexdigest()
    manifest["indexBytes"] = len(index_payload)
    (DESTINATION / "manifest.json").write_bytes(compact_json(manifest))
    print(json.dumps({"features": len(index), "countries": len(by_country), "clipLimitations": limitations,
                      "indexBytes": len(index_payload),
                      "geometryBytes": sum(record["bytes"] for record in manifest["countries"].values())}))


if __name__ == "__main__":
    main()
