"""Independent, anonymous GeoD Global usage receiver. Python standard library only."""
from __future__ import annotations

import argparse
from contextlib import contextmanager
from collections import Counter
from datetime import datetime, timezone
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import hmac
import ipaddress
import json
import os
from pathlib import Path
import re
import sqlite3
import threading
import time

SCHEMA = json.loads((Path(__file__).resolve().parents[2] / 'schemas/usage-events.json').read_text(encoding='utf-8'))
UUID = re.compile(r'^[a-f0-9]{8}-[a-f0-9]{4}-4[a-f0-9]{3}-[89ab][a-f0-9]{3}-[a-f0-9]{12}$', re.I)
VERSION = re.compile(r'^[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}(?:-[a-zA-Z0-9.-]{1,32})?$')
EVENT_KEYS = {'schemaVersion', 'product', 'id', 'installationId', 'sessionId', 'version', 'platform', 'environment', 'name', 'at', 'fields'}
DAY = 86400


def rate_limit_key(peer, forwarded):
    # Only the loopback reverse proxy may supply a client address; never persist it.
    try:
        if ipaddress.ip_address(peer).is_loopback and forwarded:
            return str(ipaddress.ip_address(forwarded))
    except ValueError:
        pass
    return peer


def validate_event(event, now=None):
    now = time.time() if now is None else now
    if not isinstance(event, dict) or not EVENT_KEYS.issubset(event) or set(event) - EVENT_KEYS - {'operationId'}:
        raise ValueError('Invalid event envelope.')
    if event['schemaVersion'] != 1 or isinstance(event['schemaVersion'], bool) or event['product'] != SCHEMA['product'] or event['name'] not in SCHEMA['events']:
        raise ValueError('Invalid product or event.')
    if any(not isinstance(event[k], str) or not UUID.fullmatch(event[k]) for k in ['id', 'installationId', 'sessionId']):
        raise ValueError('Invalid random identifier.')
    if 'operationId' in event and (not isinstance(event['operationId'], str) or not UUID.fullmatch(event['operationId'])):
        raise ValueError('Invalid operation identifier.')
    if not isinstance(event['version'], str) or not VERSION.fullmatch(event['version']) or event['platform'] not in ['desktop', 'browser'] or event['environment'] not in ['development', 'production']:
        raise ValueError('Invalid application version.')
    if not isinstance(event['at'], str) or not re.fullmatch(r'\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z', event['at']):
        raise ValueError('Invalid event time.')
    try:
        at = datetime.fromisoformat(event['at'].replace('Z', '+00:00')).timestamp()
    except (ValueError, OverflowError):
        raise ValueError('Invalid event time.') from None
    if at < now - 30 * DAY or at > now + 300:
        raise ValueError('Expired event.')
    fields = event['fields']
    if not isinstance(fields, dict) or any(key not in SCHEMA['fields'] or not isinstance(value, str) or value not in SCHEMA['fields'][key] for key, value in fields.items()):
        raise ValueError('Invalid event fields.')
    return event


class EventStore:
    def __init__(self, path):
        self.path = str(Path(path).resolve())
        Path(self.path).parent.mkdir(parents=True, exist_ok=True)
        with self.connect() as db:
            db.execute('PRAGMA journal_mode=WAL')
            db.execute('CREATE TABLE IF NOT EXISTS events (id TEXT PRIMARY KEY, received_at REAL NOT NULL, at TEXT NOT NULL, installation TEXT NOT NULL, session TEXT NOT NULL, name TEXT NOT NULL, payload TEXT NOT NULL)')
            db.execute('CREATE INDEX IF NOT EXISTS events_time ON events(received_at)')

    @contextmanager
    def connect(self):
        db = sqlite3.connect(self.path, timeout=5)
        try:
            with db:
                yield db
        finally:
            db.close()

    def prune(self, now=None):
        with self.connect() as db:
            db.execute('DELETE FROM events WHERE received_at < ?', ((time.time() if now is None else now) - 90 * DAY,))

    def accept(self, body):
        if not isinstance(body, dict) or set(body) != {'schemaVersion', 'product', 'events'} or body['schemaVersion'] != 1 or isinstance(body['schemaVersion'], bool) or body['product'] != SCHEMA['product']:
            raise ValueError('Invalid batch.')
        events = body['events']
        if not isinstance(events, list) or not 1 <= len(events) <= 50:
            raise ValueError('Invalid batch size.')
        now = time.time()
        events = [validate_event(event, now) for event in events]
        if len({event['id'] for event in events}) != len(events):
            raise ValueError('Duplicate batch identifiers.')
        with self.connect() as db:
            db.execute('DELETE FROM events WHERE received_at < ?', (now - 90 * DAY,))
            for event in events:
                db.execute('INSERT OR IGNORE INTO events VALUES (?, ?, ?, ?, ?, ?, ?)',
                           (event['id'], now, event['at'], event['installationId'], event['sessionId'], event['name'], json.dumps(event, separators=(',', ':'), sort_keys=True)))
        return {'acceptedIds': [event['id'] for event in events]}

    def report(self, environment=None):
        self.prune()
        with self.connect() as db:
            rows = [json.loads(row[0]) for row in db.execute('SELECT payload FROM events WHERE received_at >= ?', (time.time() - 90 * DAY,))]
        if environment:
            rows = [row for row in rows if row['environment'] == environment]
        count = Counter(event['name'] for event in rows)
        by_source, by_surface, by_day, failures, outcomes, durations, environments, statuses = (Counter() for _ in range(8))
        diagnostics = Counter()
        for event in rows:
            fields = event['fields']
            by_source[(fields.get('source', 'other'), event['name'])] += 1
            by_surface[(fields.get('surface', 'other'), event['name'])] += 1
            by_day[(event['at'][:10], event['name'])] += 1
            environments[(event['environment'],)] += 1
            if fields.get('result'):
                outcomes[(fields.get('source', 'other'), fields.get('action', 'other'), fields['result'])] += 1
            if fields.get('duration'):
                durations[(fields.get('source', 'other'), fields.get('action', 'other'), fields['duration'])] += 1
            if fields.get('status'):
                statuses[(fields.get('source', 'other'), fields.get('surface', 'other'), fields['status'])] += 1
            if fields.get('error'):
                failures[(fields.get('source', 'other'), fields.get('action', 'other'), fields['error'])] += 1
            if fields.get('reason') or fields.get('error'):
                diagnostics[(fields.get('source', 'other'), event['name'], fields.get('stage','unavailable'), fields.get('reason','unavailable'), fields.get('httpStatus','unknown'), fields.get('retries','unknown'), fields.get('diagnosticOrigin','unavailable'))] += 1
        def grouped(counter, names):
            return [dict(zip(names, key), count=value) for key, value in sorted(counter.items())]
        return {'product': SCHEMA['product'], 'schemaVersion': 1, 'environment': environment or 'all', 'receivedEvents': len(rows),
                'anonymousInstallations': len({row['installationId'] for row in rows}),
                'sessions': len({row['sessionId'] for row in rows}), 'events': dict(sorted(count.items())),
                'bySource': grouped(by_source, ['source', 'event']), 'bySurface': grouped(by_surface, ['surface', 'event']),
                'byDayUtc': grouped(by_day, ['date', 'event']), 'failures': grouped(failures, ['source', 'action', 'error']),
                'byEnvironment': grouped(environments, ['environment']), 'outcomes': grouped(outcomes, ['source', 'action', 'result']),
                'durations': grouped(durations, ['source', 'action', 'duration']),
                'terminalOutcomes': grouped(statuses, ['source', 'surface', 'status']),
                'diagnostics': grouped(diagnostics, ['source','event','stage','reason','httpStatus','retries','diagnosticOrigin']),
                'coverage': 'enabled-observed-events-only',
                'note': 'Installation IDs are not people. Review, request, file and task counts have different units. Missing instrumentation is not zero. No automatic cross-product identity joins.'}


def handler_for(store, *, origins=(), admin_token=''):
    allowed_origins = set(origins)
    limits, lock = {}, threading.Lock()

    class Handler(BaseHTTPRequestHandler):
        server_version = 'GeoDUsage/1'

        def log_message(self, *_args):
            pass  # No client IP, URL, user agent, or request-body access logs.

        def reply(self, status, body=None):
            data = json.dumps(body or {}, separators=(',', ':')).encode()
            self.send_response(status)
            self.send_header('Content-Type', 'application/json')
            self.send_header('Content-Length', str(len(data)))
            self.send_header('Cache-Control', 'no-store')
            self.send_header('X-Content-Type-Options', 'nosniff')
            origin = self.headers.get('Origin')
            if origin in allowed_origins:
                self.send_header('Access-Control-Allow-Origin', origin)
                self.send_header('Vary', 'Origin')
                self.send_header('Access-Control-Allow-Headers', 'Content-Type')
                self.send_header('Access-Control-Allow-Methods', 'POST, OPTIONS')
            self.end_headers()
            self.wfile.write(data)

        def do_OPTIONS(self):
            self.reply(200 if self.path == '/v1/events' and self.headers.get('Origin') in allowed_origins else 403)

        def do_POST(self):
            if self.path != '/v1/events':
                self.reply(404)
                return
            if self.headers.get('Origin') and self.headers['Origin'] not in allowed_origins:
                self.reply(403)
                return
            if self.headers.get('Content-Type', '').split(';')[0].strip() != 'application/json':
                self.reply(415)
                return
            try:
                length = int(self.headers.get('Content-Length', '0'))
                if not 1 <= length <= 65536:
                    self.reply(413)
                    return
                now = time.monotonic()
                with lock:
                    # Transient anti-abuse state, never written to the database.
                    for key, (at, _count) in list(limits.items()):
                        if now - at >= 60:
                            del limits[key]
                    ip = rate_limit_key(self.client_address[0], self.headers.get('X-Real-IP'))
                    at, count = limits.get(ip, (now, 0))
                    if count >= 120 or ip not in limits and len(limits) >= 4096:
                        self.reply(429)
                        return
                    limits[ip] = (at, count + 1)
                result = store.accept(json.loads(self.rfile.read(length)))
                self.reply(200, result)
            except (ValueError, TypeError, json.JSONDecodeError):
                self.reply(400, {'error': 'Invalid anonymous usage event.'})
            except sqlite3.Error:
                self.reply(503, {'error': 'Usage storage unavailable.'})

        def do_GET(self):
            if self.path == '/health':
                self.reply(200, {'product': SCHEMA['product'], 'ready': True})
            elif self.path == '/v1/report':
                token = self.headers.get('Authorization', '')
                if not admin_token or not hmac.compare_digest(token, 'Bearer ' + admin_token):
                    self.reply(401)
                    return
                self.reply(200, store.report())
            else:
                self.reply(404)

    return Handler


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--db', required=True)
    parser.add_argument('--port', type=int, default=4392)
    parser.add_argument('--report', action='store_true')
    parser.add_argument('--environment', choices=['development', 'production'])
    args = parser.parse_args()
    if args.report and not Path(args.db).is_file():
        print(json.dumps({'product': SCHEMA['product'], 'status': 'not_collected'}))
        return
    store = EventStore(args.db)
    if args.report:
        print(json.dumps(store.report(args.environment), ensure_ascii=False, indent=2))
        return
    origins = os.environ.get('GEOD_USAGE_ORIGINS', 'https://geod-global.laogao.xyz,http://127.0.0.1:4317,http://tauri.localhost,https://tauri.localhost,tauri://localhost').split(',')
    server = ThreadingHTTPServer(('127.0.0.1', args.port), handler_for(store, origins=origins, admin_token=os.environ.get('GEOD_USAGE_ADMIN_TOKEN', '')))
    server.daemon_threads = True
    stopped = threading.Event()
    def maintenance():
        while not stopped.wait(3600):
            store.prune()
    threading.Thread(target=maintenance, daemon=True).start()
    store.prune()
    print(f'GeoD Global usage receiver ready on 127.0.0.1:{server.server_port}', flush=True)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        stopped.set()
        server.server_close()


if __name__ == '__main__':
    main()
