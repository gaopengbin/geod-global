"""Stripe sandbox adapter. Live collection is deliberately unavailable in beta."""
import hashlib
import hmac
import json
import re

from .network import bounded_json
from .policy import PRODUCT, SKUS, ServiceError


class SandboxPayments:
    def __init__(self, client, store, key='', webhook_secret='', allowed_users=()):
        if key and not key.startswith('sk_test_'):
            raise ValueError('Only Stripe test keys are accepted before live-commerce acceptance.')
        self.client, self.store, self.key, self.secret = client, store, key, webhook_secret
        self.allowed_users = frozenset(allowed_users)
        if any(not isinstance(user, str) or not re.fullmatch(r'[a-f0-9]{32}', user) for user in self.allowed_users):
            raise ValueError('Sandbox customers must be explicitly invited Global user IDs.')

    @property
    def configured(self):
        return bool(self.key and self.secret.startswith('whsec_') and self.allowed_users)

    def authorized(self, user):
        return self.configured and user in self.allowed_users

    async def checkout(self, user, sku, idem):
        if not self.configured:
            raise ServiceError('checkout_not_available', 503)
        if not self.authorized(user):
            raise ServiceError('sandbox_invitation_required', 403)
        order = self.store.create_order(user, sku, idem)
        if order['session_id']:
            return dict(self.store.public_order(order), checkoutUrl=order['checkout_url'], mode='stripe-test')
        form = {
            'mode': 'payment', 'client_reference_id': order['id'], 'metadata[order_id]': order['id'], 'metadata[product]': PRODUCT,
            'line_items[0][price_data][currency]': 'usd', 'line_items[0][price_data][unit_amount]': str(order['amount']),
            'line_items[0][price_data][product_data][name]': 'GeoD Global sandbox · ' + sku, 'line_items[0][quantity]': '1',
            # No user-controlled redirect or automatic subscription.
            'success_url': 'https://geod-global.laogao.xyz/zh/index.html?billing=test-return',
            'cancel_url': 'https://geod-global.laogao.xyz/zh/index.html?billing=test-cancel',
        }
        result = await bounded_json(self.client, 'POST', 'https://api.stripe.com/v1/checkout/sessions', max_bytes=65536,
                                    data=form, headers={'Authorization': 'Bearer ' + self.key, 'Idempotency-Key': order['id']})
        url, session_id = result.get('url'), result.get('id')
        if (not isinstance(session_id, str) or not re.fullmatch(r'cs_test_[A-Za-z0-9]+', session_id)
                or not isinstance(url, str) or not url.startswith('https://checkout.stripe.com/') or result.get('livemode') is not False
                or result.get('client_reference_id') != order['id']):
            raise ServiceError('payment_invalid_response', 503)
        self.store.session(order['id'], session_id, url)
        return dict(self.store.public_order(order), checkoutUrl=url, mode='stripe-test')

    def verify_event(self, body, signature):
        if not self.configured:
            raise ServiceError('checkout_not_available', 503)
        try:
            pairs = [piece.split('=', 1) for piece in signature.split(',')]
            stamps = [value for key, value in pairs if key == 't']
            signatures = [value for key, value in pairs if key == 'v1']
            if len(stamps) != 1 or not stamps[0].isdigit() or abs(self.store.now() - int(stamps[0])) > 300:
                raise ValueError()
            digest = hmac.new(self.secret.encode(), stamps[0].encode() + b'.' + body, hashlib.sha256).hexdigest()
            if not any(hmac.compare_digest(digest, value) for value in signatures):
                raise ValueError()
            event = json.loads(body)
            if not re.fullmatch(r'evt_[A-Za-z0-9]+', event['id']) or event.get('livemode') is not False:
                raise ValueError()
            kind = event['type']
            if kind not in ('checkout.session.completed', 'checkout.session.async_payment_succeeded', 'checkout.session.expired'):
                return None
            session_id = event['data']['object']['id']
            if not re.fullmatch(r'cs_test_[A-Za-z0-9]+', session_id):
                raise ValueError()
        except (ValueError, TypeError, KeyError):
            raise ServiceError('invalid_payment_signature') from None
        with self.store.transaction() as db:
            previous = db.execute('SELECT * FROM payment_events WHERE id=?', (event['id'],)).fetchone()
            if previous and (previous['session_id'] != session_id or previous['kind'] != kind):
                raise ServiceError('payment_event_conflict', 409)
            db.execute("INSERT OR IGNORE INTO payment_events VALUES(?,?,?,'pending',?,NULL)", (event['id'], session_id, kind, self.store.now()))
        return event['id']

    async def reconcile(self, event_id):
        with self.store.connect() as db:
            event = db.execute('SELECT * FROM payment_events WHERE id=?', (event_id,)).fetchone()
        if not event or event['state'] == 'complete':
            return
        session = await bounded_json(self.client, 'GET', 'https://api.stripe.com/v1/checkout/sessions/' + event['session_id'],
                                     max_bytes=65536, headers={'Authorization': 'Bearer ' + self.key})
        if session.get('id') != event['session_id']:
            raise ServiceError('payment_invalid_response', 503)
        # A signed webhook is a wakeup; the payment is still verified through the provider.
        if session.get('payment_status') == 'paid':
            self.store.paid(session)
        elif event['kind'] == 'checkout.session.expired' and session.get('status') == 'expired':
            with self.store.transaction() as db:
                db.execute("UPDATE orders SET status='expired' WHERE session_id=? AND status='pending_payment'", (event['session_id'],))
        else:
            raise ServiceError('payment_pending', 409)
        with self.store.transaction() as db:
            db.execute("UPDATE payment_events SET state='complete',error=NULL WHERE id=?", (event_id,))

    async def recover(self):
        if not self.configured:
            return
        with self.store.connect() as db:
            events = [row[0] for row in db.execute("SELECT id FROM payment_events WHERE state='pending' LIMIT 20")]
        for identity in events:
            try:
                await self.reconcile(identity)
            except ServiceError as error:
                with self.store.transaction() as db:
                    db.execute('UPDATE payment_events SET error=? WHERE id=?', (error.code, identity))
        with self.store.connect() as db:
            refunds = [dict(row) for row in db.execute("SELECT * FROM refund_requests WHERE state='provider_pending' LIMIT 20")]
        for row in refunds:
            await self.refund(row['id'], row['amount'], 'recovery-worker', 'resume recorded approval')

    async def restore(self, user):
        if not self.authorized(user):
            raise ServiceError('sandbox_invitation_required', 403)
        # Recover even if the webhook never reached us. Ownership comes from the server ledger.
        with self.store.connect() as db:
            sessions = [row[0] for row in db.execute("SELECT session_id FROM orders WHERE user_id=? AND status='pending_payment' AND session_id IS NOT NULL LIMIT 10", (user,))]
        for session_id in sessions:
            session = await bounded_json(self.client, 'GET', 'https://api.stripe.com/v1/checkout/sessions/' + session_id,
                                         max_bytes=65536, headers={'Authorization': 'Bearer ' + self.key})
            if session.get('id') != session_id:
                raise ServiceError('payment_invalid_response', 503)
            if session.get('payment_status') == 'paid':
                self.store.paid(session)
        return {'orders': self.store.orders(user)}

    async def refund(self, request_id, amount, operator, reason):
        if not self.configured:
            raise ServiceError('checkout_not_available', 503)
        review = self.store.prepare_refund(request_id, amount, operator, reason)
        if review['state'] == 'completed':
            return
        if review['provider_id']:
            result = await bounded_json(self.client, 'GET', 'https://api.stripe.com/v1/refunds/' + review['provider_id'],
                                        max_bytes=65536, headers={'Authorization': 'Bearer ' + self.key})
        else:
            result = await bounded_json(self.client, 'POST', 'https://api.stripe.com/v1/refunds', max_bytes=65536,
                data={'payment_intent': review['payment_id'], 'amount': str(amount)},
                headers={'Authorization': 'Bearer ' + self.key, 'Idempotency-Key': request_id})
            if not isinstance(result.get('id'), str) or not re.fullmatch(r're_[A-Za-z0-9]+', result['id']):
                raise ServiceError('refund_invalid_response', 503)
            self.store.refund_provider(request_id, result['id'])
        self.store.refund_result(request_id, result)
