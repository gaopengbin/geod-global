"""Durable official-service authority; no client balance or Pro flag is trusted."""
import contextlib
import json
import sqlite3
import time
import uuid
from pathlib import Path

from .policy import CHECK_COST, MICROS, POLICY_VERSION, SKUS, ServiceError, canonical, fingerprint, identifier, month_after, monitor_input


SCHEMA = '''
CREATE TABLE IF NOT EXISTS customers(user_id TEXT PRIMARY KEY, created INTEGER NOT NULL, blocked INTEGER NOT NULL DEFAULT 0);
CREATE TABLE IF NOT EXISTS subscriptions(id TEXT PRIMARY KEY, user_id TEXT NOT NULL, starts INTEGER NOT NULL, ends INTEGER NOT NULL, months INTEGER NOT NULL, status TEXT NOT NULL, origin TEXT UNIQUE NOT NULL);
CREATE TABLE IF NOT EXISTS grants(id TEXT PRIMARY KEY, user_id TEXT NOT NULL, kind TEXT NOT NULL, origin TEXT UNIQUE NOT NULL, total INTEGER NOT NULL, remaining INTEGER NOT NULL CHECK(remaining>=0), reserved INTEGER NOT NULL CHECK(reserved>=0), spent INTEGER NOT NULL CHECK(spent>=0), expires INTEGER, voided INTEGER NOT NULL DEFAULT 0 CHECK(voided>=0), CHECK(total=remaining+reserved+spent+voided));
CREATE TABLE IF NOT EXISTS ledger(seq INTEGER PRIMARY KEY AUTOINCREMENT, user_id TEXT NOT NULL, event TEXT NOT NULL, kind TEXT NOT NULL, amount INTEGER NOT NULL, created INTEGER NOT NULL, detail TEXT NOT NULL, UNIQUE(event,kind));
CREATE TABLE IF NOT EXISTS quotes(id TEXT PRIMARY KEY, user_id TEXT NOT NULL, hash TEXT NOT NULL, payload TEXT NOT NULL, expires INTEGER NOT NULL, accepted TEXT UNIQUE);
CREATE TABLE IF NOT EXISTS monitors(id TEXT PRIMARY KEY, user_id TEXT NOT NULL, quote_id TEXT UNIQUE NOT NULL, config TEXT NOT NULL, state TEXT NOT NULL, next_check INTEGER NOT NULL, created INTEGER NOT NULL, last_check INTEGER, last_status TEXT, last_error TEXT, lease INTEGER NOT NULL DEFAULT 0);
CREATE TABLE IF NOT EXISTS executions(id TEXT PRIMARY KEY, user_id TEXT NOT NULL, monitor_id TEXT NOT NULL, idem TEXT NOT NULL, status TEXT NOT NULL, reserved INTEGER NOT NULL, consumed INTEGER NOT NULL DEFAULT 0, created INTEGER NOT NULL, finished INTEGER, error TEXT, result TEXT, UNIQUE(user_id,idem));
CREATE TABLE IF NOT EXISTS allocations(execution_id TEXT NOT NULL, grant_id TEXT NOT NULL, amount INTEGER NOT NULL, PRIMARY KEY(execution_id,grant_id));
CREATE TABLE IF NOT EXISTS seen(monitor_id TEXT NOT NULL, source_id TEXT NOT NULL, item_id TEXT NOT NULL, observed TEXT NOT NULL, PRIMARY KEY(monitor_id,source_id,item_id));
CREATE TABLE IF NOT EXISTS notifications(id TEXT PRIMARY KEY, user_id TEXT NOT NULL, monitor_id TEXT NOT NULL, execution_id TEXT UNIQUE NOT NULL, payload TEXT NOT NULL, created INTEGER NOT NULL, read INTEGER NOT NULL DEFAULT 0);
CREATE TABLE IF NOT EXISTS orders(id TEXT PRIMARY KEY, user_id TEXT NOT NULL, sku TEXT NOT NULL, amount INTEGER NOT NULL, currency TEXT NOT NULL, status TEXT NOT NULL, idem TEXT NOT NULL, session_id TEXT UNIQUE, payment_id TEXT UNIQUE, checkout_url TEXT, created INTEGER NOT NULL, paid INTEGER, refunded INTEGER NOT NULL DEFAULT 0, UNIQUE(user_id,idem));
CREATE TABLE IF NOT EXISTS payment_events(id TEXT PRIMARY KEY, session_id TEXT NOT NULL, kind TEXT NOT NULL, state TEXT NOT NULL, created INTEGER NOT NULL, error TEXT);
CREATE TABLE IF NOT EXISTS refund_requests(id TEXT PRIMARY KEY, user_id TEXT NOT NULL, order_id TEXT NOT NULL, reason TEXT NOT NULL, state TEXT NOT NULL, created INTEGER NOT NULL, amount INTEGER NOT NULL DEFAULT 0, provider_id TEXT, previous_block INTEGER NOT NULL DEFAULT 0);
CREATE TABLE IF NOT EXISTS rate_limits(user_id TEXT NOT NULL, window INTEGER NOT NULL, count INTEGER NOT NULL, PRIMARY KEY(user_id,window));
'''


def new_id():
    return uuid.uuid4().hex


class ClosingConnection(sqlite3.Connection):
    def __exit__(self, *args):
        try:
            return super().__exit__(*args)
        finally:
            self.close()


class Store:
    def __init__(self, path, clock=time.time):
        self.path, self.clock = str(path), clock
        Path(path).parent.mkdir(parents=True, exist_ok=True)
        with self.connect() as db:
            db.executescript(SCHEMA)
            # The unreleased first beta schema had no refund bucket. Upgrade
            # its balances without discarding reservations or payment history.
            if 'voided' not in {row[1] for row in db.execute('PRAGMA table_info(grants)')}:
                db.executescript('''BEGIN IMMEDIATE;
                ALTER TABLE grants RENAME TO grants_v1;
                CREATE TABLE grants(id TEXT PRIMARY KEY,user_id TEXT NOT NULL,kind TEXT NOT NULL,origin TEXT UNIQUE NOT NULL,total INTEGER NOT NULL,remaining INTEGER NOT NULL CHECK(remaining>=0),reserved INTEGER NOT NULL CHECK(reserved>=0),spent INTEGER NOT NULL CHECK(spent>=0),expires INTEGER,voided INTEGER NOT NULL DEFAULT 0 CHECK(voided>=0),CHECK(total=remaining+reserved+spent+voided));
                INSERT INTO grants SELECT *,0 FROM grants_v1;
                DROP TABLE grants_v1; COMMIT;''')
            fields = {row[1] for row in db.execute('PRAGMA table_info(refund_requests)')}
            for name, definition in [('amount', 'INTEGER NOT NULL DEFAULT 0'), ('provider_id', 'TEXT'), ('previous_block', 'INTEGER NOT NULL DEFAULT 0')]:
                if name not in fields:
                    db.execute(f'ALTER TABLE refund_requests ADD COLUMN {name} {definition}')
            db.execute('PRAGMA user_version=2')

    def now(self):
        return int(self.clock())

    def connect(self):
        db = sqlite3.connect(self.path, timeout=15, factory=ClosingConnection)
        db.row_factory = sqlite3.Row
        db.execute('PRAGMA foreign_keys=ON')
        db.execute('PRAGMA journal_mode=WAL')
        return db

    @contextlib.contextmanager
    def transaction(self):
        db = self.connect()
        try:
            db.execute('BEGIN IMMEDIATE')
            yield db
            db.commit()
        except BaseException:
            db.rollback()
            raise
        finally:
            db.close()

    def entry(self, db, user, event, kind, amount, detail=None):
        db.execute('INSERT OR IGNORE INTO ledger(user_id,event,kind,amount,created,detail) VALUES(?,?,?,?,?,?)', (user, event, kind, amount, self.now(), canonical(detail or {})))

    def grant(self, db, user, kind, origin, amount, expires=None):
        cursor = db.execute('INSERT OR IGNORE INTO grants VALUES(?,?,?,?,?,?,0,0,?,0)', (new_id(), user, kind, origin, amount, amount, expires))
        if cursor.rowcount:
            self.entry(db, user, origin, 'grant', amount, {'bucket': kind, 'expiresAt': expires})

    def ensure(self, user):
        with self.transaction() as db:
            db.execute('INSERT OR IGNORE INTO customers VALUES(?,?,0)', (user, self.now()))
            self.grant(db, user, 'trial', 'trial:' + user, 100 * MICROS, self.now() + 30 * 86400)
            self.monthly_grants(db, user)

    def monthly_grants(self, db, user):
        for subscription in db.execute("SELECT * FROM subscriptions WHERE user_id=? AND status='active' AND ends>?", (user, self.now())).fetchall():
            for index in range(subscription['months']):
                start = month_after(subscription['starts'], index)
                end = min(month_after(subscription['starts'], index + 1), subscription['ends'])
                if start <= self.now() < end:
                    self.grant(db, user, 'pro-month', f"subscription:{subscription['id']}:{index}", 300 * MICROS, end)

    def pro(self, db, user):
        customer = db.execute('SELECT blocked FROM customers WHERE user_id=?', (user,)).fetchone()
        if not customer or customer['blocked']:
            raise ServiceError('account_service_paused', 403)
        row = db.execute("SELECT MAX(ends) AS expires FROM subscriptions WHERE user_id=? AND status='active' AND starts<=? AND ends>?", (user, self.now(), self.now())).fetchone()
        if not row['expires']:
            raise ServiceError('pro_required', 403)
        return row['expires']

    def beta_grant(self, user, days, actor, reason):
        """Operator CLI only. There is deliberately no HTTP grant endpoint."""
        if not actor or not reason or not 1 <= days <= 366:
            raise ValueError('Operator, reason and valid duration are required.')
        self.ensure(user)
        with self.transaction() as db:
            identity = new_id()
            end = self.now() + days * 86400
            db.execute('INSERT INTO subscriptions VALUES(?,?,?,?,?,?,?)', (identity, user, self.now(), end, 12, 'active', 'operator:' + identity))
            self.entry(db, user, identity, 'beta-pro-grant', 0, {'actor': actor, 'reason': reason, 'expiresAt': end})
            self.monthly_grants(db, user)
        return identity

    def limit(self, user):
        with self.transaction() as db:
            window = self.now() // 60
            db.execute('DELETE FROM rate_limits WHERE window<?', (window - 2,))
            db.execute('INSERT INTO rate_limits VALUES(?,?,1) ON CONFLICT(user_id,window) DO UPDATE SET count=count+1', (user, window))
            count = db.execute('SELECT count FROM rate_limits WHERE user_id=? AND window=?', (user, window)).fetchone()[0]
        if count > 60:
            raise ServiceError('rate_limited', 429)

    def account(self, user):
        self.ensure(user)
        with self.connect() as db:
            try:
                expires = self.pro(db, user)
            except ServiceError:
                expires = None
            buckets = [dict(row) for row in db.execute('SELECT kind,SUM(remaining) AS availableMicros,SUM(reserved) AS reservedMicros,MIN(expires) AS expiresAt FROM grants WHERE user_id=? AND (expires IS NULL OR expires>?) GROUP BY kind', (user, self.now()))]
            recent = [dict(row) for row in db.execute('SELECT seq,kind,amount AS amountMicros,created,detail FROM ledger WHERE user_id=? ORDER BY seq DESC LIMIT 50', (user,))]
            for row in recent:
                row['detail'] = json.loads(row['detail'])
        return {'plan': 'pro' if expires else 'free', 'proExpiresAt': expires, 'creditMicrosPerCredit': MICROS, 'buckets': buckets, 'ledger': recent, 'mode': 'beta-test-credits', 'localCapabilitiesRestricted': False}

    def quote(self, user, payload):
        if not isinstance(payload, dict):
            raise ServiceError('invalid_request')
        payload = dict(payload)
        conversation = payload.pop('conversationId', None)
        if conversation is not None:
            identifier(conversation)
        payload = monitor_input(payload)
        with self.transaction() as db:
            self.pro(db, user)
            quote_id = new_id()
            expires = self.now() + 900
            content = {'service': 'imagery-monitor', 'config': payload, 'policyVersion': POLICY_VERSION, 'checkCostMicros': CHECK_COST, 'expiresAt': expires, 'conversationId': conversation}
            hash_value = fingerprint(content)
            db.execute('INSERT INTO quotes VALUES(?,?,?,?,?,NULL)', (quote_id, user, hash_value, canonical(content), expires))
        return dict(content, id=quote_id, hash=hash_value, mode='beta-test-credits', startsDownloads=False)

    def accept(self, user, quote_id, hash_value):
        with self.transaction() as db:
            quote = db.execute('SELECT * FROM quotes WHERE id=? AND user_id=?', (quote_id, user)).fetchone()
            if not quote:
                raise ServiceError('not_found', 404)
            if quote['hash'] != hash_value:
                raise ServiceError('quote_changed', 409)
            if quote['accepted']:
                return self.monitor(db, user, quote['accepted'])
            if quote['expires'] <= self.now():
                raise ServiceError('quote_expired', 409)
            self.pro(db, user)
            count = db.execute("SELECT COUNT(*) FROM monitors WHERE user_id=? AND state!='deleted'", (user,)).fetchone()[0]
            if count >= 10:
                raise ServiceError('monitor_limit', 409)
            config = json.loads(quote['payload'])['config']
            monitor = new_id()
            db.execute('INSERT INTO monitors(id,user_id,quote_id,config,state,next_check,created) VALUES(?,?,?,?,?,?,?)', (monitor, user, quote_id, canonical(config), 'active', self.now(), self.now()))
            db.execute('UPDATE quotes SET accepted=? WHERE id=?', (monitor, quote_id))
            self.entry(db, user, quote_id, 'monitor-confirmed', 0, {'monitorId': monitor, 'inputHash': quote['hash'], 'dailyCreditLimit': config['dailyCreditLimit'], 'totalCreditLimit': config['totalCreditLimit']})
            return self.monitor(db, user, monitor)

    def pending_quotes(self, user):
        with self.connect() as db:
            return [dict(json.loads(row['payload']), id=row['id'], hash=row['hash'], mode='beta-test-credits', startsDownloads=False)
                    for row in db.execute('SELECT * FROM quotes WHERE user_id=? AND accepted IS NULL AND expires>? ORDER BY expires DESC LIMIT 10', (user, self.now()))]

    def monitor(self, db, user, monitor_id):
        row = db.execute("SELECT * FROM monitors WHERE id=? AND user_id=? AND state!='deleted'", (monitor_id, user)).fetchone()
        if not row:
            raise ServiceError('not_found', 404)
        result = dict(row)
        result['config'] = json.loads(result['config'])
        result['runs'] = [self.execution(row) for row in db.execute('SELECT * FROM executions WHERE monitor_id=? AND user_id=? ORDER BY created DESC LIMIT 10', (monitor_id, user))]
        result.pop('user_id')
        result.pop('lease')
        return result

    @staticmethod
    def execution(row):
        result = {key: row[key] for key in ('id', 'status', 'reserved', 'consumed', 'created', 'finished', 'error')}
        result['result'] = json.loads(row['result']) if row['result'] else None
        if result['result']:
            result['result']['items'] = result['result']['items'][:20]
        return result

    def list_monitors(self, user):
        with self.connect() as db:
            return [self.monitor(db, user, row[0]) for row in db.execute("SELECT id FROM monitors WHERE user_id=? AND state!='deleted' ORDER BY created DESC", (user,)).fetchall()]

    def control(self, user, monitor_id, action):
        if action not in ('pause', 'resume', 'delete'):
            raise ServiceError('invalid_request')
        with self.transaction() as db:
            self.monitor(db, user, monitor_id)
            if action == 'resume':
                self.pro(db, user)
            # Accepted work may finish, but deletion/pause suppresses future runs.
            db.execute('UPDATE monitors SET state=?,next_check=? WHERE id=?', ({'pause': 'paused', 'resume': 'active', 'delete': 'deleted'}[action], self.now(), monitor_id))
            self.entry(db, user, new_id(), 'monitor-' + action, 0, {'monitorId': monitor_id})
        return {'ok': True}

    def reserve(self, db, user, execution):
        remaining = CHECK_COST
        grants = db.execute('SELECT * FROM grants WHERE user_id=? AND (expires IS NULL OR expires>?) AND remaining>0 ORDER BY expires IS NULL,expires,id', (user, self.now())).fetchall()
        if sum(row['remaining'] for row in grants) < remaining:
            raise ServiceError('credits_required', 402)
        for row in grants:
            amount = min(remaining, row['remaining'])
            db.execute('UPDATE grants SET remaining=remaining-?,reserved=reserved+? WHERE id=?', (amount, amount, row['id']))
            db.execute('INSERT INTO allocations VALUES(?,?,?)', (execution, row['id'], amount))
            remaining -= amount
            if not remaining:
                break
        self.entry(db, user, execution, 'reserve', CHECK_COST)

    def enqueue(self, user, monitor_id, idem):
        identifier(idem)
        self.ensure(user)
        with self.transaction() as db:
            previous = db.execute('SELECT * FROM executions WHERE user_id=? AND idem=?', (user, idem)).fetchone()
            if previous:
                if previous['monitor_id'] != monitor_id:
                    raise ServiceError('idempotency_conflict', 409)
                return self.execution(previous)
            self.pro(db, user)
            monitor = self.monitor(db, user, monitor_id)
            if monitor['state'] != 'active':
                raise ServiceError('monitor_paused', 409)
            active = db.execute("SELECT 1 FROM executions WHERE monitor_id=? AND status IN ('queued','running')", (monitor_id,)).fetchone()
            if active:
                raise ServiceError('check_in_progress', 409)
            used = db.execute("SELECT COALESCE(SUM(CASE WHEN status IN ('queued','running') THEN reserved ELSE consumed END),0) AS total, COALESCE(SUM(CASE WHEN created>=? THEN CASE WHEN status IN ('queued','running') THEN reserved ELSE consumed END ELSE 0 END),0) AS today FROM executions WHERE monitor_id=?", (self.now() // 86400 * 86400, monitor_id)).fetchone()
            if used['total'] + CHECK_COST > monitor['config']['totalCreditLimit'] * MICROS or used['today'] + CHECK_COST > monitor['config']['dailyCreditLimit'] * MICROS:
                raise ServiceError('budget_limit', 409)
            execution = new_id()
            self.reserve(db, user, execution)
            db.execute('INSERT INTO executions(id,user_id,monitor_id,idem,status,reserved,created) VALUES(?,?,?,?,?,?,?)', (execution, user, monitor_id, idem, 'queued', CHECK_COST, self.now()))
            db.execute('UPDATE monitors SET next_check=? WHERE id=?', (self.now() + monitor['config']['intervalHours'] * 3600, monitor_id))
            return self.execution(db.execute('SELECT * FROM executions WHERE id=?', (execution,)).fetchone())

    def claim(self):
        with self.transaction() as db:
            row = db.execute("SELECT e.*,m.config FROM executions e JOIN monitors m ON m.id=e.monitor_id WHERE e.status='queued' ORDER BY e.created LIMIT 1").fetchone()
            if not row:
                return None
            # A newly cancelled queued request has no vendor work to reconcile.
            state = db.execute('SELECT state FROM monitors WHERE id=?', (row['monitor_id'],)).fetchone()[0]
            if state != 'active':
                self.finish_in(db, row['id'], None, 'monitor_paused')
                return None
            db.execute("UPDATE executions SET status='running' WHERE id=?", (row['id'],))
            db.execute('UPDATE monitors SET lease=? WHERE id=?', (self.now() + 120, row['monitor_id']))
            return dict(row, config=json.loads(row['config']))

    def finish_in(self, db, execution_id, result, error=None):
        row = db.execute('SELECT * FROM executions WHERE id=?', (execution_id,)).fetchone()
        if not row or row['status'] not in ('queued', 'running'):
            return
        consumed = CHECK_COST if result is not None and result.get('complete') is True and not error else 0
        for allocation in db.execute('SELECT * FROM allocations WHERE execution_id=?', (execution_id,)).fetchall():
            charge = allocation['amount'] if consumed else 0
            db.execute('UPDATE grants SET reserved=reserved-?,spent=spent+?,remaining=remaining+? WHERE id=?', (allocation['amount'], charge, allocation['amount'] - charge, allocation['grant_id']))
        self.entry(db, row['user_id'], execution_id, 'settle', consumed)
        self.entry(db, row['user_id'], execution_id, 'release', CHECK_COST - consumed)
        if result is not None:
            new_items = []
            source = json.loads(db.execute('SELECT config FROM monitors WHERE id=?', (row['monitor_id'],)).fetchone()[0])['sourceId']
            for item in result['items']:
                added = db.execute('INSERT OR IGNORE INTO seen VALUES(?,?,?,?)', (row['monitor_id'], source, item['id'], item['datetime'])).rowcount
                if added:
                    new_items.append(item)
            result = dict(result, newCount=len(new_items))
            if new_items:
                note = dict(result, items=new_items[:20], sourceId=source, monitorId=row['monitor_id'])
                db.execute('INSERT OR IGNORE INTO notifications VALUES(?,?,?,?,?,?,0)', (new_id(), row['user_id'], row['monitor_id'], execution_id, canonical(note), self.now()))
        state = 'failed' if error else 'completed' if consumed else 'partial'
        db.execute('UPDATE executions SET status=?,consumed=?,finished=?,error=?,result=? WHERE id=?', (state, consumed, self.now(), error, canonical(result) if result is not None else None, execution_id))
        db.execute('UPDATE monitors SET last_check=?,last_status=?,last_error=?,lease=0 WHERE id=?', (self.now(), state, error, row['monitor_id']))

    def finish(self, execution_id, result, error=None):
        with self.transaction() as db:
            self.finish_in(db, execution_id, result, error)

    def recover(self):
        with self.transaction() as db:
            for row in db.execute("SELECT e.id FROM executions e JOIN monitors m ON m.id=e.monitor_id WHERE e.status='running' AND m.lease<=?", (self.now(),)).fetchall():
                # This service only queries public catalogs. Platform absorbs unknown interrupted work.
                self.finish_in(db, row['id'], None, 'interrupted_check')

    def due(self):
        with self.connect() as db:
            return [dict(row) for row in db.execute("SELECT id,user_id,next_check FROM monitors WHERE state='active' AND next_check<=? LIMIT 100", (self.now(),))]

    def postpone(self, monitor_id, error):
        with self.transaction() as db:
            db.execute('UPDATE monitors SET next_check=?,last_error=? WHERE id=?', (self.now() + 3600, error, monitor_id))

    def inbox(self, user):
        with self.connect() as db:
            return [dict(id=row['id'], created=row['created'], read=bool(row['read']), **json.loads(row['payload'])) for row in db.execute('SELECT * FROM notifications WHERE user_id=? ORDER BY created DESC LIMIT 50', (user,))]

    def read(self, user, ids):
        if not isinstance(ids, list) or len(ids) > 50:
            raise ServiceError('invalid_request')
        with self.transaction() as db:
            for value in ids:
                identifier(value)
                db.execute('UPDATE notifications SET read=1 WHERE id=? AND user_id=?', (value, user))
        return {'ok': True}

    def create_order(self, user, sku, idem):
        if sku not in SKUS:
            raise ServiceError('invalid_request')
        identifier(idem)
        with self.transaction() as db:
            customer = db.execute('SELECT blocked FROM customers WHERE user_id=?', (user,)).fetchone()
            if not customer or customer['blocked']:
                raise ServiceError('account_service_paused', 403)
            old = db.execute('SELECT * FROM orders WHERE user_id=? AND idem=?', (user, idem)).fetchone()
            if old:
                if old['sku'] != sku:
                    raise ServiceError('idempotency_conflict', 409)
                return dict(old)
            if SKUS[sku]['months']:
                if db.execute("SELECT 1 FROM subscriptions WHERE user_id=? AND status='active' AND ends>?", (user, self.now())).fetchone():
                    raise ServiceError('subscription_already_active', 409)
                if db.execute("SELECT 1 FROM orders WHERE user_id=? AND sku LIKE 'pro-%' AND status='pending_payment'", (user,)).fetchone():
                    raise ServiceError('checkout_pending', 409)
            order = new_id()
            db.execute('INSERT INTO orders(id,user_id,sku,amount,currency,status,idem,created) VALUES(?,?,?,?,?,?,?,?)', (order, user, sku, SKUS[sku]['priceMinor'], 'usd', 'pending_payment', idem, self.now()))
            return dict(db.execute('SELECT * FROM orders WHERE id=?', (order,)).fetchone())

    def session(self, order, session_id, url):
        with self.transaction() as db:
            db.execute('UPDATE orders SET session_id=?,checkout_url=? WHERE id=? AND session_id IS NULL', (session_id, url, order))

    def paid(self, session):
        with self.transaction() as db:
            order = db.execute('SELECT * FROM orders WHERE session_id=?', (session.get('id'),)).fetchone()
            if not order:
                raise ServiceError('unrecognized_payment', 409)
            metadata = session.get('metadata') or {}
            if (session.get('payment_status') != 'paid' or session.get('status') != 'complete' or session.get('mode') != 'payment'
                    or session.get('currency') != order['currency'] or type(session.get('amount_total')) is not int or session['amount_total'] != order['amount']
                    or metadata.get('order_id') != order['id'] or session.get('client_reference_id') != order['id'] or session.get('livemode') is not False
                    or not isinstance(session.get('payment_intent'), str)):
                raise ServiceError('payment_not_verified', 409)
            if order['status'] == 'fulfilled':
                return
            if order['status'] not in ('pending_payment', 'expired'):
                raise ServiceError('order_not_payable', 409)
            sku = SKUS[order['sku']]
            if sku['credits']:
                self.grant(db, order['user_id'], 'purchased', 'order:' + order['id'], sku['credits'] * MICROS)
            else:
                # Prepaid fixed term: never silently enable automatic renewal.
                existing = db.execute("SELECT MAX(ends) FROM subscriptions WHERE user_id=? AND status='active'", (order['user_id'],)).fetchone()[0]
                start = max(self.now(), existing or 0)
                db.execute('INSERT INTO subscriptions VALUES(?,?,?,?,?,?,?)', (new_id(), order['user_id'], start, month_after(start, sku['months']), sku['months'], 'active', 'order:' + order['id']))
                self.monthly_grants(db, order['user_id'])
            db.execute("UPDATE orders SET status='fulfilled',payment_id=?,paid=? WHERE id=?", (session['payment_intent'], self.now(), order['id']))
            self.entry(db, order['user_id'], order['id'], 'payment-fulfilled-test', 0, {'sku': order['sku'], 'amountMinor': order['amount'], 'currency': order['currency']})

    def orders(self, user):
        with self.connect() as db:
            return [self.public_order(row) for row in db.execute('SELECT * FROM orders WHERE user_id=? ORDER BY created DESC LIMIT 50', (user,))]

    @staticmethod
    def public_order(row):
        return {key: row[key] for key in ('id', 'sku', 'amount', 'currency', 'status', 'created', 'paid', 'refunded')}

    def refund_request(self, user, order_id, reason):
        if not isinstance(reason, str) or not 1 <= len(reason.strip()) <= 500:
            raise ServiceError('invalid_request')
        with self.transaction() as db:
            order = db.execute("SELECT * FROM orders WHERE id=? AND user_id=? AND status='fulfilled'", (order_id, user)).fetchone()
            if not order:
                raise ServiceError('not_found', 404)
            previous = db.execute("SELECT * FROM refund_requests WHERE user_id=? AND order_id=? AND state='pending_review'", (user, order_id)).fetchone()
            identity = previous['id'] if previous else new_id()
            if not previous:
                db.execute('INSERT INTO refund_requests(id,user_id,order_id,reason,state,created) VALUES(?,?,?,?,?,?)', (identity, user, order_id, reason.strip(), 'pending_review', self.now()))
            return {'id': identity, 'state': 'pending_review'}

    def prepare_refund(self, request_id, amount, operator, reason):
        """Operator-reviewed sandbox refund. No HTTP approval route exists."""
        if not operator or not reason or type(amount) is not int or amount <= 0:
            raise ServiceError('invalid_refund_review')
        with self.transaction() as db:
            row = db.execute('SELECT * FROM refund_requests WHERE id=?', (request_id,)).fetchone()
            if not row:
                raise ServiceError('not_found', 404)
            order = db.execute('SELECT * FROM orders WHERE id=?', (row['order_id'],)).fetchone()
            if row['state'] in ('provider_pending', 'completed'):
                if row['amount'] != amount:
                    raise ServiceError('idempotency_conflict', 409)
                return dict(row, payment_id=order['payment_id'])
            if row['state'] != 'pending_review' or amount > order['amount'] - order['refunded']:
                raise ServiceError('refund_review_required', 409)
            customer = db.execute('SELECT blocked FROM customers WHERE user_id=?', (row['user_id'],)).fetchone()
            if customer['blocked']:
                raise ServiceError('account_service_paused', 409)
            sku = SKUS[order['sku']]
            if sku['credits']:
                grant = db.execute('SELECT * FROM grants WHERE origin=?', ('order:' + order['id'],)).fetchone()
                if not grant or grant['reserved'] or amount * MICROS > grant['remaining']:
                    raise ServiceError('refund_review_required', 409)
            else:
                used = db.execute("SELECT 1 FROM executions WHERE user_id=? AND created>=? AND status IN ('queued','running','completed')", (row['user_id'], order['paid'])).fetchone()
                if used or amount != order['amount']:
                    raise ServiceError('refund_review_required', 409)
            # Freeze new official service work until the provider result is known;
            # an administrator cannot race withdrawal with credit spending.
            db.execute('UPDATE customers SET blocked=1 WHERE user_id=?', (row['user_id'],))
            db.execute("UPDATE refund_requests SET state='provider_pending',amount=?,previous_block=? WHERE id=?", (amount, customer['blocked'], request_id))
            self.entry(db, row['user_id'], request_id, 'refund-approved-test', 0, {'operator': operator, 'reason': reason, 'amountMinor': amount})
            return dict(row, amount=amount, state='provider_pending', payment_id=order['payment_id'])

    def refund_provider(self, request_id, provider_id):
        with self.transaction() as db:
            db.execute('UPDATE refund_requests SET provider_id=? WHERE id=? AND provider_id IS NULL', (provider_id, request_id))

    def refund_result(self, request_id, refund):
        with self.transaction() as db:
            row = db.execute('SELECT * FROM refund_requests WHERE id=?', (request_id,)).fetchone()
            if not row or row['state'] != 'provider_pending':
                return
            order = db.execute('SELECT * FROM orders WHERE id=?', (row['order_id'],)).fetchone()
            if (refund.get('id') != row['provider_id'] or refund.get('payment_intent') != order['payment_id']
                    or refund.get('amount') != row['amount'] or refund.get('currency') != order['currency']):
                raise ServiceError('refund_not_verified', 409)
            if refund.get('status') == 'succeeded':
                sku = SKUS[order['sku']]
                if sku['credits']:
                    amount = row['amount'] * MICROS
                    changed = db.execute('UPDATE grants SET remaining=remaining-?,voided=voided+? WHERE origin=? AND remaining>=? AND reserved=0', (amount, amount, 'order:' + order['id'], amount)).rowcount
                    if not changed:
                        raise ServiceError('refund_reconciliation_required', 409)
                else:
                    subscriptions = [r[0] for r in db.execute('SELECT id FROM subscriptions WHERE origin=?', ('order:' + order['id'],))]
                    for subscription in subscriptions:
                        if db.execute('SELECT 1 FROM grants WHERE origin LIKE ? AND reserved>0', ('subscription:' + subscription + ':%',)).fetchone():
                            raise ServiceError('refund_reconciliation_required', 409)
                        db.execute('UPDATE grants SET voided=voided+remaining,remaining=0 WHERE origin LIKE ?', ('subscription:' + subscription + ':%',))
                    db.execute("UPDATE subscriptions SET status='refunded' WHERE origin=?", ('order:' + order['id'],))
                db.execute("UPDATE orders SET refunded=refunded+?,status=CASE WHEN refunded+?=amount THEN 'refunded' ELSE 'fulfilled' END WHERE id=?", (row['amount'], row['amount'], order['id']))
                db.execute("UPDATE refund_requests SET state='completed' WHERE id=?", (request_id,))
                self.entry(db, row['user_id'], request_id, 'refund-verified-test', 0, {'amountMinor': row['amount'], 'providerRefundId': row['provider_id'], 'bankSettlementConfirmed': False})
            elif refund.get('status') in ('failed', 'canceled'):
                db.execute("UPDATE refund_requests SET state='provider_failed' WHERE id=?", (request_id,))
                self.entry(db, row['user_id'], request_id, 'refund-failed-test', 0)
            else:
                return
            db.execute('UPDATE customers SET blocked=? WHERE user_id=?', (row['previous_block'], row['user_id']))
