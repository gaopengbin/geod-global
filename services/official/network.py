"""Fixed upstreams, bounded bodies and server-side Global session validation."""
import json
import math
import re
from datetime import datetime, timedelta, timezone
from urllib.parse import parse_qs, urlsplit

import httpx

from .policy import PRODUCT, SOURCES, ServiceError


async def bounded_json(client, method, url, max_bytes=4 * 1024 * 1024, **kwargs):
    async with client.stream(method, url, **kwargs) as response:
        if response.status_code >= 400:
            raise ServiceError('upstream_unavailable', 503)
        if response.status_code not in (200, 201):
            raise ServiceError('upstream_invalid_response', 503)
        content = bytearray()
        async for part in response.aiter_bytes():
            content.extend(part)
            if len(content) > max_bytes:
                raise ServiceError('upstream_response_too_large', 503)
        try:
            result = json.loads(content)
        except (ValueError, UnicodeError):
            raise ServiceError('upstream_invalid_response', 503) from None
        if not isinstance(result, dict):
            raise ServiceError('upstream_invalid_response', 503)
        return result


class GlobalIdentity:
    def __init__(self, origin, client, clock):
        parsed = urlsplit(origin)
        if (parsed.username or parsed.password or parsed.query or parsed.fragment or parsed.path not in ('', '/')
                or not parsed.hostname or (parsed.scheme != 'https' and not (parsed.scheme == 'http' and parsed.hostname == '127.0.0.1'))):
            raise ValueError('A fixed HTTPS Global account origin is required.')
        self.origin, self.client, self.clock = origin.rstrip('/'), client, clock

    async def authenticate(self, request):
        raw = request.headers.get('authorization', '')
        if not re.fullmatch(r'Bearer [A-Za-z0-9_-]{43}', raw):
            raise ServiceError('sign_in_required', 401)
        # No cache: logout/revocation at the identity authority applies to every new request.
        async with self.client.stream('GET', self.origin + '/api/desktop-auth/me', headers={'Authorization': raw, 'Accept': 'application/json'}) as response:
            if response.status_code == 401:
                raise ServiceError('sign_in_required', 401)
            if response.status_code != 200:
                raise ServiceError('identity_unavailable', 503)
            content = bytearray()
            async for chunk in response.aiter_bytes():
                content.extend(chunk)
                if len(content) > 16384:
                    raise ServiceError('identity_unavailable', 503)
        try:
            data = json.loads(content)
            user = data['user']
            valid = (data['product'] == PRODUCT and data['version'] == 1 and isinstance(user, dict)
                     and isinstance(user.get('id'), str) and re.fullmatch(r'[a-f0-9]{32}', user['id'])
                     and type(user.get('expiresAt')) is int and user['expiresAt'] > self.clock()
                     and user.get('provider') in ('email', 'google', 'github')
                     and (user['provider'] != 'email' or user.get('emailVerified') is True))
        except (ValueError, KeyError, TypeError):
            valid = False
        if not valid:
            raise ServiceError('identity_unavailable', 503)
        return user['id']


class Catalog:
    def __init__(self, client, clock):
        self.client, self.clock = client, clock

    async def search(self, config):
        source = SOURCES[config['sourceId']]
        end = datetime.fromtimestamp(self.clock(), timezone.utc)
        start = end - timedelta(days=config['lookbackDays'])
        iso = lambda date: date.isoformat(timespec='seconds').replace('+00:00', 'Z')
        query = {'collections': [source['collection']], 'intersects': config['geometry'], 'datetime': iso(start) + '/' + iso(end),
                 'query': {'eo:cloud_cover': {'lte': config['cloudMax']}}, 'limit': 100, 'sortby': [{'field': 'properties.datetime', 'direction': 'desc'}]}
        items, ids, tokens, cursor, complete = [], set(), set(), None, False
        for page in range(10):
            payload = dict(query)
            if cursor:
                payload[cursor[0]] = cursor[1]
            response = await bounded_json(self.client, 'POST', source['url'], json=payload)
            features = response.get('features')
            if response.get('type') != 'FeatureCollection' or not isinstance(features, list) or len(features) > 100:
                raise ServiceError('catalog_invalid_response', 503)
            for item in features:
                try:
                    props = item['properties']
                    item_id, observed, cloud = item['id'], props['datetime'], props.get('eo:cloud_cover')
                    stamp = datetime.fromisoformat(observed.replace('Z', '+00:00'))
                    if (not isinstance(item_id, str) or not 1 <= len(item_id) <= 300 or item.get('collection') != source['collection']
                            or stamp.tzinfo is None or not start <= stamp <= end or type(cloud) not in (int, float)
                            or not math.isfinite(cloud) or not 0 <= cloud <= config['cloudMax']):
                        continue
                    if item_id not in ids:
                        ids.add(item_id)
                        items.append({'id': item_id, 'datetime': observed, 'cloud': cloud})
                except (KeyError, TypeError, ValueError, AttributeError):
                    raise ServiceError('catalog_invalid_response', 503) from None
            links = response.get('links', [])
            if not isinstance(links, list):
                raise ServiceError('catalog_invalid_response', 503)
            following = [link for link in links if isinstance(link, dict) and link.get('rel') == 'next']
            if not following:
                complete = True
                break
            if len(following) != 1:
                raise ServiceError('catalog_invalid_response', 503)
            link = following[0]
            target, expected = urlsplit(link.get('href', '')), urlsplit(source['url'])
            if (target.scheme, target.netloc, target.path) != (expected.scheme, expected.netloc, expected.path):
                raise ServiceError('catalog_pagination_refused', 503)
            next_body = link.get('body') or {}
            if not isinstance(next_body, dict):
                raise ServiceError('catalog_pagination_refused', 503)
            # Earth Search uses `next`, Planetary Computer uses `token`. Only
            # copy the cursor; never accept changed geometry/filter fields.
            key = 'next' if 'next' in next_body else 'token'
            next_token = next_body.get(key) or parse_qs(target.query).get(key, [None])[0]
            next_cursor = (key, next_token)
            if not isinstance(next_token, str) or not 1 <= len(next_token) <= 4096 or next_cursor in tokens:
                raise ServiceError('catalog_pagination_refused', 503)
            tokens.add(next_cursor)
            cursor = next_cursor
        return {'items': items, 'count': len(items), 'complete': complete, 'pages': page + 1, 'searchedAt': int(self.clock()),
                'from': iso(start), 'to': iso(end), 'cloudMeaning': 'scene-average-not-area-cloud',
                'coverageVerified': False, 'downloadsStarted': False, 'sourceId': config['sourceId']}


def http_client():
    return httpx.AsyncClient(timeout=httpx.Timeout(20, connect=8), follow_redirects=False, headers={'User-Agent': 'GeoD-Global-Official/aggregation-beta-1'})
