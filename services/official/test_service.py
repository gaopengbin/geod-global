"""Synthetic identity/provider/payment tests; never send mail or charge cards."""
import asyncio
import hashlib
import hmac
import json
import tempfile
import unittest
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

import httpx

from .app import PREFIX, create_app
from .network import Catalog, GlobalIdentity
from .payments import SandboxPayments
from .policy import MICROS, PRODUCT, ServiceError, month_after
from .store import Store

USER, OTHER = 'a' * 32, 'b' * 32
TOKEN, OTHER_TOKEN = 'a' * 43, 'b' * 43
GEOMETRY = {'type': 'Polygon', 'coordinates': [[[116, 40], [116.1, 40], [116.1, 40.1], [116, 40.1], [116, 40]]]}
CONFIG = {'name': 'Synthetic Beijing AOI', 'sourceId': 'earth-search', 'geometry': GEOMETRY, 'cloudMax': 5, 'lookbackDays': 30, 'intervalHours': 24, 'dailyCreditLimit': 5, 'totalCreditLimit': 100}


class ServiceTests(unittest.IsolatedAsyncioTestCase):
    async def asyncSetUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.now = 1791532800
        self.revoked = False
        self.calls, self.features, self.extra_links = [], [], []
        self.stripe_sessions, self.stripe_creations = {}, 0
        self.stripe_refunds = {}
        self.store = Store(Path(self.temp.name) / 'billing.sqlite3', lambda: self.now)
        self.upstream = httpx.AsyncClient(transport=httpx.MockTransport(self.provider))
        auth = GlobalIdentity('https://account.example', self.upstream, lambda: self.now)
        self.payments = SandboxPayments(self.upstream, self.store, 'sk_test_fixture', 'whsec_fixture', (USER,))
        self.app = create_app(self.store, self.upstream, auth, payments=self.payments, start_worker=False)
        self.api = httpx.AsyncClient(transport=httpx.ASGITransport(app=self.app), base_url='https://service.example')

    async def asyncTearDown(self):
        await self.api.aclose()
        await self.upstream.aclose()
        self.temp.cleanup()

    def provider(self, request):
        self.calls.append((str(request.url), request.headers.get('authorization'), request.content))
        if request.url.host == 'account.example':
            token = request.headers.get('authorization')
            user = USER if token == 'Bearer ' + TOKEN else OTHER if token == 'Bearer ' + OTHER_TOKEN else None
            if not user or self.revoked:
                return httpx.Response(401, json={'user': None})
            return httpx.Response(200, json={'product': PRODUCT, 'version': 1, 'user': {'id': user, 'expiresAt': self.now + 60, 'provider': 'email', 'emailVerified': True}})
        if request.url.host == 'api.stripe.com':
            if request.url.path.startswith('/v1/refunds'):
                if request.method == 'POST':
                    from urllib.parse import parse_qs
                    values = parse_qs(request.content.decode())
                    rid = 're_' + request.headers['idempotency-key']
                    self.stripe_refunds[rid] = {'id': rid, 'amount': int(values['amount'][0]), 'currency': 'usd', 'payment_intent': values['payment_intent'][0], 'status': 'succeeded'}
                    return httpx.Response(200, json=self.stripe_refunds[rid])
                return httpx.Response(200, json=self.stripe_refunds[request.url.path.split('/')[-1]])
            if request.method == 'POST':
                from urllib.parse import parse_qs
                values = parse_qs(request.content.decode())
                sid = 'cs_test_' + request.headers['idempotency-key']
                self.stripe_creations += 1
                self.stripe_sessions[sid] = {'id': sid, 'url': 'https://checkout.stripe.com/c/pay/' + sid,
                    'livemode': False, 'mode': 'payment', 'status': 'open', 'payment_status': 'unpaid',
                    'currency': 'usd', 'amount_total': int(values['line_items[0][price_data][unit_amount]'][0]),
                    'client_reference_id': values['client_reference_id'][0], 'metadata': {'order_id': values['metadata[order_id]'][0]},
                    'payment_intent': 'pi_' + request.headers['idempotency-key']}
                return httpx.Response(200, json=self.stripe_sessions[sid])
            return httpx.Response(200, json=self.stripe_sessions[request.url.path.split('/')[-1]])
        if request.url.host == 'earth-search.aws.element84.com':
            return httpx.Response(200, json={'type': 'FeatureCollection', 'features': self.features, 'links': self.extra_links})
        raise AssertionError('Unexpected upstream: ' + str(request.url))

    async def call(self, method, path, data=None, other=False, headers=None):
        return await self.api.request(method, PREFIX + path, json=data, headers={'Authorization': 'Bearer ' + (OTHER_TOKEN if other else TOKEN), **(headers or {})})

    async def monitor(self, **changes):
        self.store.beta_grant(USER, 30, 'test-operator', 'synthetic fixture')
        quote = (await self.call('POST', '/monitor-quotes', dict(CONFIG, **changes))).json()['quote']
        response = await self.call('POST', '/monitor-quotes/' + quote['id'] + '/accept', {'hash': quote['hash']})
        self.assertEqual(response.status_code, 200, response.text)
        return response.json()['monitor'], quote

    def feature(self, item='synthetic-a', cloud=2):
        from datetime import datetime, timezone
        return {'id': item, 'collection': 'sentinel-2-l2a', 'properties': {'datetime': datetime.fromtimestamp(self.now - 86400, timezone.utc).isoformat().replace('+00:00', 'Z'), 'eo:cloud_cover': cloud}}

    async def test_identity_revocation_and_browser_origin_are_enforced(self):
        self.assertEqual((await self.api.get(PREFIX + '/account')).status_code, 401)
        self.assertEqual((await self.call('GET', '/account')).status_code, 200)
        self.revoked = True
        self.assertEqual((await self.call('GET', '/account')).status_code, 401)
        self.assertEqual((await self.call('GET', '/account', headers={'Origin': 'https://attacker.example'})).status_code, 403)

    async def test_free_client_cannot_claim_pro_or_set_price(self):
        self.assertEqual((await self.call('POST', '/monitor-quotes', CONFIG)).json()['error'], 'pro_required')
        self.assertEqual((await self.call('POST', '/monitor-quotes', dict(CONFIG, isPro=True))).status_code, 400)
        self.assertEqual((await self.call('POST', '/checkout', {'sku': 'credits-1000', 'idempotencyKey': 'buy-key-001', 'amount': 1})).status_code, 400)
        with self.store.connect() as db:
            self.assertEqual(db.execute('SELECT COUNT(*) FROM orders').fetchone()[0], 0)

    async def test_trial_once_across_restarts(self):
        for _ in range(3):
            await self.call('GET', '/account')
        restored = Store(self.store.path, lambda: self.now)
        restored.ensure(USER)
        with restored.connect() as db:
            self.assertEqual(db.execute("SELECT SUM(total) FROM grants WHERE kind='trial'").fetchone()[0], 100 * MICROS)

    async def test_quote_hash_expiry_ownership_and_idempotency(self):
        monitor, quote = await self.monitor()
        path = '/monitor-quotes/' + quote['id'] + '/accept'
        self.assertEqual((await self.call('POST', path, {'hash': quote['hash']})).json()['monitor']['id'], monitor['id'])
        self.assertEqual((await self.call('POST', path, {'hash': 'changed'})).status_code, 409)
        self.assertEqual((await self.call('POST', path, {'hash': quote['hash']}, other=True)).status_code, 404)
        newer = (await self.call('POST', '/monitor-quotes', CONFIG)).json()['quote']
        self.now += 901
        self.assertEqual((await self.call('POST', '/monitor-quotes/' + newer['id'] + '/accept', {'hash': newer['hash']})).json()['error'], 'quote_expired')

    async def test_conversation_review_is_bound_preserves_geometry_and_does_not_start_work(self):
        self.store.beta_grant(USER, 30, 'operator', 'fixture')
        conversation = 'c' * 32
        quote = (await self.call('POST', '/monitor-quotes', dict(CONFIG, conversationId=conversation))).json()['quote']
        self.assertEqual(quote['conversationId'], conversation)
        self.assertEqual(quote['config']['geometry'], GEOMETRY)
        self.assertEqual((await self.call('GET', '/monitor-quotes')).json()['quotes'][0]['id'], quote['id'])
        self.assertEqual((await self.call('GET', '/monitor-quotes', other=True)).json()['quotes'], [])
        self.assertEqual((await self.call('GET', '/monitors')).json()['monitors'], [])
        with self.store.connect() as db:
            self.assertEqual(db.execute('SELECT COUNT(*) FROM executions').fetchone()[0], 0)
        self.assertEqual((await self.call('POST', '/monitor-quotes', dict(CONFIG, conversationId='bad/id'))).status_code, 400)

    async def test_real_server_work_deduplicates_alerts_and_settles_once(self):
        monitor, _ = await self.monitor()
        self.features = [self.feature()]
        await self.app.state.worker.tick()
        first = (await self.call('GET', '/monitors')).json()['monitors'][0]
        self.assertEqual(first['runs'][0]['consumed'], MICROS)
        self.assertEqual(first['runs'][0]['result']['newCount'], 1)
        self.assertFalse(first['runs'][0]['result']['coverageVerified'])
        self.assertFalse(first['runs'][0]['result']['downloadsStarted'])
        alert = (await self.call('GET', '/notifications')).json()['notifications'][0]
        run = await self.call('POST', '/monitors/' + monitor['id'] + '/run', {'idempotencyKey': 'manual-test-1'})
        repeated = await self.call('POST', '/monitors/' + monitor['id'] + '/run', {'idempotencyKey': 'manual-test-1'})
        self.assertEqual(run.json()['execution']['id'], repeated.json()['execution']['id'])
        await self.app.state.worker.tick()
        self.assertEqual(len((await self.call('GET', '/notifications')).json()['notifications']), 1)
        await self.call('POST', '/notifications/read', {'ids': [alert['id']]}, other=True)
        self.assertFalse((await self.call('GET', '/notifications')).json()['notifications'][0]['read'])
        await self.call('POST', '/notifications/read', {'ids': [alert['id']]})
        self.assertTrue((await self.call('GET', '/notifications')).json()['notifications'][0]['read'])
        self.assertEqual((await self.call('GET', '/monitors', other=True)).json()['monitors'], [])

    async def test_vendor_error_releases_same_bucket(self):
        await self.monitor()
        self.features = [{'malformed': True}]
        before = self.store.account(USER)['buckets']
        await self.app.state.worker.tick()
        after = self.store.account(USER)['buckets']
        self.assertEqual(before, after)
        self.assertEqual(self.store.list_monitors(USER)[0]['runs'][0]['status'], 'failed')

    async def test_upstream_next_link_cannot_exfiltrate_token_or_query_private_network(self):
        await self.monitor()
        self.extra_links = [{'rel': 'next', 'href': 'http://127.0.0.1/private', 'body': {'token': 'secret'}}]
        await self.app.state.worker.tick()
        self.assertEqual(self.store.list_monitors(USER)[0]['runs'][0]['error'], 'catalog_pagination_refused')
        self.assertTrue(all(url.startswith(('https://account.example', 'https://earth-search.aws.element84.com')) for url, _, _ in self.calls))
        self.assertTrue(all(auth is None for url, auth, _ in self.calls if 'earth-search' in url))

    async def test_pause_revocation_and_budget_stop_new_work(self):
        monitor, _ = await self.monitor(dailyCreditLimit=1, totalCreditLimit=1)
        await self.app.state.worker.tick()
        response = await self.call('POST', '/monitors/' + monitor['id'] + '/run', {'idempotencyKey': 'over-budget-1'})
        self.assertEqual(response.json()['error'], 'budget_limit')
        self.assertEqual((await self.call('POST', '/monitors/' + monitor['id'] + '/control', {'action': 'pause'}, other=True)).status_code, 404)
        await self.call('POST', '/monitors/' + monitor['id'] + '/control', {'action': 'pause'})
        self.assertEqual((await self.call('POST', '/monitors/' + monitor['id'] + '/run', {'idempotencyKey': 'paused-key-1'})).json()['error'], 'monitor_paused')
        self.now += 31 * 86400
        self.assertEqual((await self.call('POST', '/monitors/' + monitor['id'] + '/control', {'action': 'resume'})).json()['error'], 'pro_required')
        self.assertFalse(self.store.account(USER)['localCapabilitiesRestricted'])

    async def test_concurrent_reservations_and_restart_recovery(self):
        monitor, _ = await self.monitor()
        with ThreadPoolExecutor(max_workers=4) as pool:
            runs = list(pool.map(lambda _: self.store.enqueue(USER, monitor['id'], 'same-reserve-1'), range(8)))
        self.assertEqual(len({run['id'] for run in runs}), 1)
        claim = self.store.claim()
        self.now += 121
        recovered = Store(self.store.path, lambda: self.now)
        recovered.recover()
        recovered.recover()
        account = recovered.account(USER)
        self.assertEqual(sum(bucket['reservedMicros'] for bucket in account['buckets']), 0)
        self.assertEqual(recovered.list_monitors(USER)[0]['runs'][0]['consumed'], 0)
        with recovered.connect() as db:
            self.assertEqual(db.execute("SELECT COUNT(*) FROM ledger WHERE event=? AND kind='release'", (claim['id'],)).fetchone()[0], 1)

    async def test_invalid_geometry_and_oversized_request(self):
        self.store.beta_grant(USER, 30, 'operator', 'fixture')
        bad = dict(CONFIG, geometry={'type': 'Polygon', 'coordinates': [[[0, 0], [float('inf'), 1], [0, 0], [0, 0]]]})
        with self.assertRaises(ServiceError):
            self.store.quote(USER, bad)
        self.assertEqual((await self.call('POST', '/monitor-quotes', dict(CONFIG, sourceId='http://private'))).status_code, 400)
        response = await self.call('POST', '/monitor-quotes', dict(CONFIG, name='x' * 70000))
        self.assertEqual(response.status_code, 413)

    async def checkout(self, sku='credits-1000', key='checkout-id-1'):
        response = await self.call('POST', '/checkout', {'sku': sku, 'idempotencyKey': key})
        self.assertEqual(response.status_code, 200, response.text)
        order = response.json()['order']
        with self.store.connect() as db:
            sid = db.execute('SELECT session_id FROM orders WHERE id=?', (order['id'],)).fetchone()[0]
        return order, self.stripe_sessions[sid]

    def signed(self, session, event='evt_fixture1', age=0):
        body = json.dumps({'id': event, 'type': 'checkout.session.completed', 'livemode': False, 'data': {'object': {'id': session['id']}}}).encode()
        stamp = str(self.now - age)
        signature = 't=' + stamp + ',v1=' + hmac.new(b'whsec_fixture', stamp.encode() + b'.' + body, hashlib.sha256).hexdigest()
        return body, signature

    async def test_signed_payment_verified_amount_and_duplicate_fulfillment(self):
        order, session = await self.checkout()
        duplicate, _ = await self.checkout()
        self.assertEqual(order['id'], duplicate['id'])
        self.assertEqual(self.stripe_creations, 1)
        body, signature = self.signed(session)
        response = await self.api.post(PREFIX + '/webhooks/stripe', content=body, headers={'Stripe-Signature': signature})
        self.assertEqual(response.status_code, 200)
        await self.payments.recover()
        self.assertEqual(self.store.orders(USER)[0]['status'], 'pending_payment')
        session.update(status='complete', payment_status='paid')
        session['amount_total'] = 1
        await self.payments.recover()
        self.assertEqual(self.store.orders(USER)[0]['status'], 'pending_payment')
        session['amount_total'] = 1000
        await self.payments.recover()
        await self.api.post(PREFIX + '/webhooks/stripe', content=body, headers={'Stripe-Signature': signature})
        await self.payments.recover()
        self.assertEqual(self.store.orders(USER)[0]['status'], 'fulfilled')
        self.assertEqual(next(b for b in self.store.account(USER)['buckets'] if b['kind'] == 'purchased')['availableMicros'], 1000 * MICROS)
        self.assertEqual(self.store.account(USER)['plan'], 'free')
        self.assertEqual((await self.call('GET', '/orders', other=True)).json()['orders'], [])

    async def test_bad_signature_and_expired_signature_never_fulfill(self):
        _, session = await self.checkout()
        session.update(status='complete', payment_status='paid')
        body, signature = self.signed(session, age=301)
        self.assertEqual((await self.api.post(PREFIX + '/webhooks/stripe', content=body, headers={'Stripe-Signature': signature})).status_code, 400)
        body, signature = self.signed(session)
        self.assertEqual((await self.api.post(PREFIX + '/webhooks/stripe', content=body + b' ', headers={'Stripe-Signature': signature})).status_code, 400)
        self.assertEqual(self.store.orders(USER)[0]['status'], 'pending_payment')

    async def test_restore_lost_webhook_and_annual_grants(self):
        _, session = await self.checkout('pro-yearly')
        session.update(status='complete', payment_status='paid')
        await self.call('POST', '/restore-purchases', {})
        first = self.store.account(USER)
        self.assertEqual(first['plan'], 'pro')
        self.assertEqual(next(b for b in first['buckets'] if b['kind'] == 'pro-month')['availableMicros'], 300 * MICROS)
        self.now = month_after(self.now, 1)
        for _ in range(3):
            second = self.store.account(USER)
        self.assertEqual(next(b for b in second['buckets'] if b['kind'] == 'pro-month')['availableMicros'], 300 * MICROS)
        self.assertEqual(len([entry for entry in second['ledger'] if entry['kind'] == 'grant' and entry['detail'].get('bucket') == 'pro-month']), 2)

    async def test_checkout_off_without_merchant_and_live_keys_rejected(self):
        with self.assertRaises(ValueError):
            SandboxPayments(self.upstream, self.store, 'sk_live_fake', 'whsec_fake')
        app = create_app(self.store, self.upstream, auth=self.app.routes[0].endpoint, payments=SandboxPayments(self.upstream, self.store), start_worker=False)
        client = httpx.AsyncClient(transport=httpx.ASGITransport(app=app), base_url='https://service.example')
        try:
            data = (await client.get(PREFIX + '/capabilities')).json()
            self.assertFalse(data['checkout']['available'])
            self.assertFalse(data['livePayments'])
            self.assertFalse(data['imageryProcurement'])
        finally:
            await client.aclose()

    async def test_configured_test_provider_cannot_grant_real_resources_to_uninvited_account(self):
        response = await self.call('POST', '/checkout', {'sku':'pro-monthly','idempotencyKey':'uninvited-order-1'}, other=True)
        self.assertEqual(response.json()['error'], 'sandbox_invitation_required')
        self.assertEqual(self.store.orders(OTHER), [])
        self.assertFalse((await self.call('GET','/account',other=True)).json()['account']['sandboxCheckoutAllowed'])
        self.assertEqual(self.stripe_creations,0)

    async def test_refund_request_is_owned_review_not_bank_success(self):
        order, session = await self.checkout()
        session.update(status='complete', payment_status='paid')
        await self.payments.restore(USER)
        path = '/orders/' + order['id'] + '/refund-request'
        self.assertEqual((await self.call('POST', path, {'reason': 'test'}, other=True)).status_code, 404)
        first = (await self.call('POST', path, {'reason': 'test'})).json()['refund']
        second = (await self.call('POST', path, {'reason': 'test'})).json()['refund']
        self.assertEqual(first, second)
        self.assertEqual(first['state'], 'pending_review')
        self.assertEqual(self.store.orders(USER)[0]['status'], 'fulfilled')

    async def test_operator_partial_refund_withdraws_only_original_unused_purchased_bucket(self):
        order, session = await self.checkout()
        session.update(status='complete', payment_status='paid')
        await self.payments.restore(USER)
        request = self.store.refund_request(USER, order['id'], 'synthetic refund')
        await self.payments.refund(request['id'], 500, 'test-operator', 'approved test partial refund')
        await self.payments.refund(request['id'], 500, 'test-operator', 'retry same approval')
        self.assertEqual(len(self.stripe_refunds), 1)
        buckets = self.store.account(USER)['buckets']
        self.assertEqual(next(b for b in buckets if b['kind'] == 'purchased')['availableMicros'], 500 * MICROS)
        self.assertEqual(next(b for b in buckets if b['kind'] == 'trial')['availableMicros'], 100 * MICROS)
        self.assertEqual(self.store.orders(USER)[0]['refunded'], 500)
        with self.store.connect() as db:
            self.assertEqual(db.execute('SELECT blocked FROM customers WHERE user_id=?', (USER,)).fetchone()[0], 0)
            self.assertEqual(db.execute("SELECT total-(remaining+reserved+spent+voided) FROM grants WHERE kind='purchased'").fetchone()[0], 0)

    async def test_pending_refund_freezes_new_official_usage_until_provider_confirms(self):
        order, session = await self.checkout('pro-monthly')
        session.update(status='complete', payment_status='paid')
        await self.payments.restore(USER)
        request = self.store.refund_request(USER, order['id'], 'synthetic refund')
        review = self.store.prepare_refund(request['id'], 1900, 'test-operator', 'test full unused term')
        with self.assertRaises(ServiceError):
            self.store.quote(USER, CONFIG)
        with self.store.connect() as db:
            self.assertEqual(db.execute('SELECT blocked FROM customers WHERE user_id=?', (USER,)).fetchone()[0], 1)
        await self.payments.refund(request['id'], 1900, 'test-operator', 'resume recorded approval')
        self.assertEqual(self.store.account(USER)['plan'], 'free')
        self.assertFalse(any(b['kind']=='pro-month' and b['availableMicros'] for b in self.store.account(USER)['buckets']))
        self.assertEqual(self.store.orders(USER)[0]['status'], 'refunded')

    async def test_server_does_not_accept_pagination_filter_mutation(self):
        self.store.beta_grant(USER, 30, 'operator', 'fixture')
        bodies = []
        def pages(request):
            body = json.loads(request.content)
            bodies.append(body)
            if len(bodies) == 1:
                return httpx.Response(200, json={'type':'FeatureCollection','features':[self.feature()], 'links':[{'rel':'next','href':'https://earth-search.aws.element84.com/v1/search','body':{'next':'native-cursor','intersects':{'type':'Point','coordinates':[0,0]},'query':{}}}]})
            return httpx.Response(200, json={'type':'FeatureCollection','features':[], 'links':[]})
        async with httpx.AsyncClient(transport=httpx.MockTransport(pages)) as client:
            result = await Catalog(client, lambda:self.now).search(CONFIG)
        self.assertTrue(result['complete'])
        self.assertEqual(bodies[1]['intersects'], GEOMETRY)
        self.assertEqual(bodies[1]['query'], {'eo:cloud_cover':{'lte':5}})
        self.assertEqual(bodies[1]['next'], 'native-cursor')


if __name__ == '__main__':
    unittest.main()
