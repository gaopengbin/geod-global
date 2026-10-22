import copy
from datetime import datetime, timezone
import http.client
import json
from pathlib import Path
import sqlite3
import tempfile
import threading
import time
import unittest
import uuid
from http.server import ThreadingHTTPServer
from server import EventStore, handler_for, rate_limit_key


def event(name='search_completed'):
    return {'schemaVersion': 1, 'product': 'geod-global', 'id': str(uuid.uuid4()), 'installationId': str(uuid.uuid4()),
            'sessionId': str(uuid.uuid4()), 'version': '0.2.0-rc.5', 'platform': 'desktop', 'environment':'development', 'name': name,
            'at': datetime.now(timezone.utc).isoformat(timespec='milliseconds').replace('+00:00', 'Z'),
            'fields': {'surface': 'manual_explore', 'channel': 'manual', 'source': 'planetary-naip', 'result': 'success', 'duration': '1_3s'}}


class ReceiverTests(unittest.TestCase):
    def test_diagnostics_survive_http_and_report_without_free_text(self):
        value=event('preview_failed')
        value['fields'].update(stage='preview_tile',reason='upstream_unavailable',httpStatus='503',retries='2',diagnosticOrigin='explicit',error='network')
        self.assertEqual(self.request('POST','/v1/events',self.batch([value]))[0],200)
        report=self.request('GET','/v1/report',headers={'Authorization':'Bearer test-only-admin'})[1]
        self.assertEqual(report['diagnostics'][0]['stage'],'preview_tile')
        self.assertEqual(report['diagnostics'][0]['httpStatus'],'503')
        for field in ['stage','reason','httpStatus','retries','diagnosticOrigin']:
            bad=copy.deepcopy(value);bad['id']=str(uuid.uuid4());bad['fields'][field]='https://private.test/?token=secret'
            self.assertEqual(self.request('POST','/v1/events',self.batch([bad]))[0],400)

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.store = EventStore(Path(self.temp.name) / 'global.sqlite')
        self.server = ThreadingHTTPServer(('127.0.0.1', 0), handler_for(self.store, origins=['http://127.0.0.1:4317'], admin_token='test-only-admin'))
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.thread.start()

    def tearDown(self):
        self.server.shutdown()
        self.server.server_close()
        self.thread.join()
        self.temp.cleanup()

    def request(self, method, path, body=None, headers=None):
        connection = http.client.HTTPConnection('127.0.0.1', self.server.server_port, timeout=5)
        connection.request(method, path, json.dumps(body) if body is not None else None,
                           {'Content-Type': 'application/json', **(headers or {})})
        response = connection.getresponse()
        result = response.status, json.loads(response.read())
        connection.close()
        return result

    @staticmethod
    def batch(value):
        return {'schemaVersion': 1, 'product': 'geod-global', 'events': value}

    def test_real_http_roundtrip_and_retry_deduplication(self):
        value = event()
        for _ in range(2):
            status, reply = self.request('POST', '/v1/events', self.batch([value]))
            self.assertEqual(status, 200)
            self.assertEqual(reply['acceptedIds'], [value['id']])
        self.assertEqual(self.request('GET', '/v1/report')[0], 401)
        status, report = self.request('GET', '/v1/report', headers={'Authorization': 'Bearer test-only-admin'})
        self.assertEqual(status, 200)
        self.assertEqual(report['receivedEvents'], 1)
        self.assertEqual(report['events']['search_completed'], 1)
        self.assertEqual(report['bySurface'][0]['surface'], 'manual_explore')
        self.assertEqual(self.store.report('production')['receivedEvents'], 0)

    def test_rejects_sensitive_fields_and_other_products_atomically(self):
        for field in ['message', 'apiKey', 'email', 'bounds', 'sceneId', 'projectName', 'path', 'url']:
            value = event()
            value['fields'][field] = 'secret'
            self.assertEqual(self.request('POST', '/v1/events', self.batch([event(), value]))[0], 400)
        wrong = event()
        wrong['product'] = 'geod-web'
        self.assertEqual(self.request('POST', '/v1/events', self.batch([wrong]))[0], 400)
        self.assertEqual(self.store.report()['receivedEvents'], 0)

    def test_rejects_free_text_categories_invalid_identifiers_and_timestamps(self):
        for patch in [{'id': '../private-file'}, {'at': '2000-01-01T00:00:00.000Z'}, {'version': 'private machine name'}, {'fields': {'source': 'https://user:secret@private.test'}}, {'unexpected': 'secret'}]:
            value = {**event(), **patch}
            self.assertEqual(self.request('POST', '/v1/events', self.batch([value]))[0], 400)

    def test_distinguishes_agent_scenes_and_retains_only_90_days(self):
        values = [event() for _ in range(3)]
        for value, surface in zip(values, ['manual_explore', 'agent_explore', 'agent_home']):
            value['fields']['surface'] = surface
            value['fields']['channel'] = 'manual' if surface == 'manual_explore' else 'agent'
        self.store.accept(self.batch(values))
        self.assertEqual(len(self.store.report()['bySurface']), 3)
        with self.store.connect() as db:
            db.execute('UPDATE events SET received_at = ?', (time.time() - 91 * 86400,))
        self.assertEqual(self.store.report()['receivedEvents'], 0)

    def test_bounds_batches_and_rejects_unapproved_browser_origins(self):
        self.assertEqual(self.request('POST', '/v1/events', self.batch([event() for _ in range(51)]))[0], 400)
        self.assertEqual(self.request('POST', '/v1/events', self.batch([event()]), {'Origin': 'https://unrelated.test'})[0], 403)
        self.assertEqual(self.request('POST', '/v1/events', self.batch([event()]), {'Origin': 'http://127.0.0.1:4317'})[0], 200)

    def test_proxy_rate_limit_keeps_clients_separate_without_trusting_remote_headers(self):
        self.assertEqual(rate_limit_key('127.0.0.1', '192.0.2.1'), '192.0.2.1')
        self.assertEqual(rate_limit_key('127.0.0.1', 'invalid'), '127.0.0.1')
        self.assertEqual(rate_limit_key('192.0.2.2', '192.0.2.1'), '192.0.2.2')
        body = self.batch([event()])
        for _ in range(120):
            self.assertEqual(self.request('POST', '/v1/events', body, {'X-Real-IP': '192.0.2.1'})[0], 200)
        self.assertEqual(self.request('POST', '/v1/events', body, {'X-Real-IP': '192.0.2.1'})[0], 429)
        self.assertEqual(self.request('POST', '/v1/events', body, {'X-Real-IP': '192.0.2.2'})[0], 200)


if __name__ == '__main__':
    unittest.main()
