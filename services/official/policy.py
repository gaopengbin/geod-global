"""Versioned server policy. Prices are sandbox hypotheses, not live offers."""
import calendar
import hashlib
import json
import math
import re
from datetime import datetime, timezone

PRODUCT = 'xyz.laogao.geod.global'
MICROS = 1_000_000
POLICY_VERSION = 'aggregation-beta-1'
CHECK_COST = MICROS
SOURCES = {
    'earth-search': {'name': 'Earth Search · Sentinel-2 L2A', 'url': 'https://earth-search.aws.element84.com/v1/search', 'collection': 'sentinel-2-l2a'},
    'planetary-landsat': {'name': 'Planetary Computer · Landsat', 'url': 'https://planetarycomputer.microsoft.com/api/stac/v1/search', 'collection': 'landsat-c2-l2'},
}
SKUS = {
    'pro-monthly': {'priceMinor': 1900, 'months': 1, 'credits': 0},
    'pro-yearly': {'priceMinor': 14900, 'months': 12, 'credits': 0},
    'credits-1000': {'priceMinor': 1000, 'months': 0, 'credits': 1000},
    'credits-3000': {'priceMinor': 3000, 'months': 0, 'credits': 3000},
}


class ServiceError(Exception):
    def __init__(self, code, status=400):
        self.code, self.status = code, status
        super().__init__(code)


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':'), ensure_ascii=False, allow_nan=False)


def fingerprint(value):
    return hashlib.sha256(canonical(value).encode()).hexdigest()


def identifier(value):
    if not isinstance(value, str) or not re.fullmatch(r'[a-zA-Z0-9_-]{8,100}', value):
        raise ServiceError('invalid_request')
    return value


def integer(value, low, high):
    if type(value) is not int or not low <= value <= high:
        raise ServiceError('invalid_request')
    return value


def month_after(stamp, count):
    date = datetime.fromtimestamp(stamp, timezone.utc)
    month = date.year * 12 + date.month - 1 + count
    year, index = divmod(month, 12)
    return int(date.replace(year=year, month=index + 1, day=min(date.day, calendar.monthrange(year, index + 1)[1])).timestamp())


def monitor_input(data):
    allowed = {'name', 'sourceId', 'geometry', 'cloudMax', 'lookbackDays', 'intervalHours', 'dailyCreditLimit', 'totalCreditLimit'}
    if not isinstance(data, dict) or set(data) != allowed:
        raise ServiceError('invalid_request')
    if not isinstance(data['name'], str) or not 1 <= len(data['name'].strip()) <= 120 or data['sourceId'] not in SOURCES:
        raise ServiceError('invalid_request')
    geometry = data['geometry']
    if not isinstance(geometry, dict) or set(geometry) != {'type', 'coordinates'} or geometry['type'] not in ('Polygon', 'MultiPolygon'):
        raise ServiceError('invalid_geometry')
    polygons = [geometry['coordinates']] if geometry['type'] == 'Polygon' else geometry['coordinates']
    if not isinstance(polygons, list) or not 1 <= len(polygons) <= 64:
        raise ServiceError('invalid_geometry')
    points = []
    for polygon in polygons:
        if not isinstance(polygon, list) or not 1 <= len(polygon) <= 64:
            raise ServiceError('invalid_geometry')
        for ring in polygon:
            if not isinstance(ring, list) or len(ring) < 4 or ring[0] != ring[-1]:
                raise ServiceError('invalid_geometry')
            for point in ring:
                if (not isinstance(point, list) or len(point) != 2 or any(type(v) not in (float, int) or not math.isfinite(v) for v in point)
                        or not -180 <= point[0] <= 180 or not -90 <= point[1] <= 90):
                    raise ServiceError('invalid_geometry')
                points.append(point)
    if len(points) > 2000:
        raise ServiceError('area_too_complex')
    west, east = min(p[0] for p in points), max(p[0] for p in points)
    south, north = min(p[1] for p in points), max(p[1] for p in points)
    # Limit server cost, and avoid silently treating a dateline polygon as its complement.
    if east <= west or north <= south or east - west > 30 or north - south > 30:
        raise ServiceError('area_too_large')
    normalized = dict(data, name=data['name'].strip())
    for field, lo, hi in [('cloudMax', 0, 100), ('lookbackDays', 1, 90), ('intervalHours', 1, 168), ('dailyCreditLimit', 1, 100), ('totalCreditLimit', 1, 10000)]:
        integer(data[field], lo, hi)
    return normalized
