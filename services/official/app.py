"""Run separately from the desktop; reverse-proxy only /api/services/v1/."""
import asyncio
import contextlib
import json
import os
import sqlite3
import logging
from contextlib import asynccontextmanager

import httpx
from starlette.applications import Starlette
from starlette.responses import JSONResponse
from starlette.routing import Route

from .network import Catalog, GlobalIdentity, http_client
from .payments import SandboxPayments
from .policy import CHECK_COST, MICROS, POLICY_VERSION, PRODUCT, SKUS, SOURCES, ServiceError, identifier
from .store import Store

PREFIX = '/api/services/v1'


class Worker:
    def __init__(self, store, catalog, payments):
        self.store, self.catalog, self.payments = store, catalog, payments

    async def tick(self):
        await asyncio.to_thread(self.store.recover)
        for row in await asyncio.to_thread(self.store.due):
            try:
                await asyncio.to_thread(self.store.enqueue, row['user_id'], row['id'], f"scheduled-{row['id']}-{row['next_check']}")
            except ServiceError as error:
                await asyncio.to_thread(self.store.postpone, row['id'], error.code)
        for _ in range(10):
            run = await asyncio.to_thread(self.store.claim)
            if not run:
                break
            try:
                result = await asyncio.wait_for(self.catalog.search(run['config']), timeout=90)
                await asyncio.to_thread(self.store.finish, run['id'], result)
            except ServiceError as error:
                await asyncio.to_thread(self.store.finish, run['id'], None, error.code)
            except (httpx.HTTPError, TimeoutError, ValueError, KeyError, TypeError):
                await asyncio.to_thread(self.store.finish, run['id'], None, 'catalog_unavailable')
        await self.payments.recover()

    async def loop(self):
        while True:
            try:
                await self.tick()
            except (httpx.HTTPError, ServiceError, sqlite3.Error):
                logging.getLogger(__name__).warning('Official worker paused this cycle; durable work will be retried.')
            await asyncio.sleep(5)


def create_app(store=None, client=None, auth=None, catalog=None, payments=None, start_worker=True):
    store = store or Store(os.environ.get('GEOD_SERVICE_DB', '.geod-service/official.sqlite3'))
    owned_client = client is None
    client = client or http_client()
    auth = auth or GlobalIdentity(os.environ.get('GEOD_ACCOUNT_ORIGIN', 'https://geod-global.laogao.xyz'), client, store.clock)
    catalog = catalog or Catalog(client, store.clock)
    test_customers = tuple(user.strip() for user in os.environ.get('GEOD_SERVICE_TEST_CUSTOMERS', '').split(',') if user.strip())
    payments = payments or SandboxPayments(client, store, os.environ.get('GEOD_STRIPE_TEST_KEY', ''), os.environ.get('GEOD_STRIPE_TEST_WEBHOOK_SECRET', ''), test_customers)
    worker = Worker(store, catalog, payments)

    @asynccontextmanager
    async def lifespan(app):
        task = asyncio.create_task(worker.loop()) if start_worker else None
        try:
            yield
        finally:
            if task:
                task.cancel()
                with contextlib.suppress(asyncio.CancelledError):
                    await task
            if owned_client:
                await client.aclose()

    async def payload(request):
        if request.headers.get('content-type', '').split(';')[0] != 'application/json':
            raise ServiceError('invalid_request')
        content = await body(request, 65536)
        try:
            data = json.loads(content)
        except (ValueError, UnicodeError):
            raise ServiceError('invalid_request') from None
        if not isinstance(data, dict):
            raise ServiceError('invalid_request')
        return data

    async def body(request, limit):
        result = bytearray()
        async def read():
            async for part in request.stream():
                result.extend(part)
                if len(result) > limit:
                    raise ServiceError('request_too_large', 413)
        await asyncio.wait_for(read(), timeout=10)
        return bytes(result)

    def handler(function, public=False):
        async def endpoint(request):
            try:
                # Desktop bearer API has no browser cookie or cross-origin credential path.
                if request.headers.get('origin'):
                    raise ServiceError('browser_origin_refused', 403)
                if public:
                    user = None
                else:
                    user = await auth.authenticate(request)
                    await asyncio.to_thread(store.ensure, user)
                    await asyncio.to_thread(store.limit, user)
                result = await function(request, user)
                return JSONResponse(dict(product=PRODUCT, version=1, **result), headers={'Cache-Control': 'no-store'})
            except ServiceError as error:
                return JSONResponse({'product': PRODUCT, 'version': 1, 'error': error.code}, error.status, headers={'Cache-Control': 'no-store'})
            except (httpx.HTTPError, TimeoutError):
                return JSONResponse({'product': PRODUCT, 'version': 1, 'error': 'service_unavailable'}, 503)
            except sqlite3.Error:
                return JSONResponse({'product': PRODUCT, 'version': 1, 'error': 'service_unavailable'}, 503)
            except (ValueError, KeyError, TypeError):
                return JSONResponse({'product': PRODUCT, 'version': 1, 'error': 'invalid_request'}, 400)
        return endpoint

    def exact(data, fields):
        if set(data) != set(fields):
            raise ServiceError('invalid_request')
        return data

    async def capabilities(request, user):
        return {'mode': 'beta-test-credits', 'imageryProcurement': False, 'livePayments': False,
                'sources': [{'id': key, 'name': source['name']} for key, source in SOURCES.items()],
                'services': [{'id': 'imagery-monitor', 'requiresPro': True, 'released': True, 'checkCostMicros': CHECK_COST}],
                'limits': {'monitors': 10, 'pageItems': 100, 'pagesPerCheck': 10},
                'checkout': {'available': payments.configured, 'mode': 'stripe-test' if payments.configured else 'unconfigured', 'automaticRenewal': False,
                             'products': [dict(id=key, **value, purchasable=payments.configured, currency='usd') for key, value in SKUS.items()]},
                'policyVersion': POLICY_VERSION, 'creditMicrosPerCredit': MICROS}

    async def account(request, user):
        result = await asyncio.to_thread(store.account, user)
        result['sandboxCheckoutAllowed'] = bool(payments.authorized(user))
        return {'account': result}

    async def monitors(request, user):
        return {'monitors': await asyncio.to_thread(store.list_monitors, user)}

    async def quote(request, user):
        return {'quote': await asyncio.to_thread(store.quote, user, await payload(request))}

    async def quotes(request, user):
        return {'quotes': await asyncio.to_thread(store.pending_quotes, user)}

    async def accept(request, user):
        data = exact(await payload(request), ['hash'])
        return {'monitor': await asyncio.to_thread(store.accept, user, identifier(request.path_params['id']), data['hash'])}

    async def control(request, user):
        data = exact(await payload(request), ['action'])
        return await asyncio.to_thread(store.control, user, identifier(request.path_params['id']), data['action'])

    async def run(request, user):
        data = exact(await payload(request), ['idempotencyKey'])
        return {'execution': await asyncio.to_thread(store.enqueue, user, identifier(request.path_params['id']), data['idempotencyKey'])}

    async def inbox(request, user):
        return {'notifications': await asyncio.to_thread(store.inbox, user)}

    async def read(request, user):
        data = exact(await payload(request), ['ids'])
        return await asyncio.to_thread(store.read, user, data['ids'])

    async def checkout(request, user):
        data = exact(await payload(request), ['sku', 'idempotencyKey'])
        return {'order': await payments.checkout(user, data['sku'], data['idempotencyKey'])}

    async def orders(request, user):
        return {'orders': await asyncio.to_thread(store.orders, user)}

    async def restore(request, user):
        exact(await payload(request), [])
        if not payments.configured:
            raise ServiceError('checkout_not_available', 503)
        return await payments.restore(user)

    async def refund(request, user):
        data = exact(await payload(request), ['reason'])
        return {'refund': await asyncio.to_thread(store.refund_request, user, identifier(request.path_params['id']), data['reason'])}

    async def webhook(request, user):
        event_id = payments.verify_event(await body(request, 65536), request.headers.get('stripe-signature', ''))
        # Persist first. Recovery handles delivery retries and provider outages.
        return {'ok': True, 'accepted': bool(event_id)}

    routes = [
        Route(PREFIX + '/capabilities', handler(capabilities, public=True)),
        Route(PREFIX + '/account', handler(account)), Route(PREFIX + '/monitors', handler(monitors)),
        Route(PREFIX + '/monitor-quotes', handler(quote), methods=['POST']),
        Route(PREFIX + '/monitor-quotes', handler(quotes)),
        Route(PREFIX + '/monitor-quotes/{id}/accept', handler(accept), methods=['POST']),
        Route(PREFIX + '/monitors/{id}/control', handler(control), methods=['POST']),
        Route(PREFIX + '/monitors/{id}/run', handler(run), methods=['POST']),
        Route(PREFIX + '/notifications', handler(inbox)), Route(PREFIX + '/notifications/read', handler(read), methods=['POST']),
        Route(PREFIX + '/checkout', handler(checkout), methods=['POST']), Route(PREFIX + '/orders', handler(orders)),
        Route(PREFIX + '/restore-purchases', handler(restore), methods=['POST']),
        Route(PREFIX + '/orders/{id}/refund-request', handler(refund), methods=['POST']),
        Route(PREFIX + '/webhooks/stripe', handler(webhook, public=True), methods=['POST']),
    ]
    app = Starlette(routes=routes, lifespan=lifespan)
    app.state.store, app.state.worker = store, worker
    return app


def main():
    import uvicorn
    uvicorn.run(create_app(), host='127.0.0.1', port=int(os.environ.get('GEOD_SERVICE_PORT', '4319')), log_level='warning', access_log=False)


if __name__ == '__main__':
    main()
