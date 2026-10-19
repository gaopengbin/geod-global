"""Local operator commands; never expose these through desktop or HTTP."""
import argparse
import re
import asyncio
import os

from .store import Store


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--db', required=True)
    sub = parser.add_subparsers(dest='command', required=True)
    grant = sub.add_parser('grant-beta-pro')
    grant.add_argument('--user', required=True)
    grant.add_argument('--days', type=int, default=30)
    grant.add_argument('--operator', required=True)
    grant.add_argument('--reason', required=True)
    revoke = sub.add_parser('revoke-pro')
    revoke.add_argument('--user', required=True)
    revoke.add_argument('--operator', required=True)
    revoke.add_argument('--reason', required=True)
    refund = sub.add_parser('process-test-refund')
    refund.add_argument('--request', required=True)
    refund.add_argument('--amount-minor', type=int, required=True)
    refund.add_argument('--operator', required=True)
    refund.add_argument('--reason', required=True)
    args = parser.parse_args()
    target = args.request if args.command == 'process-test-refund' else args.user
    if not re.fullmatch(r'[a-f0-9]{32}', target) or not args.operator.strip() or not args.reason.strip():
        parser.error('A valid Global user ID, operator and reason are required.')
    store = Store(args.db)
    if args.command == 'grant-beta-pro':
        identity = store.beta_grant(args.user, args.days, args.operator, args.reason)
        print('Beta service entitlement granted:', identity)
    elif args.command == 'revoke-pro':
        with store.transaction() as db:
            db.execute("UPDATE subscriptions SET status='revoked' WHERE user_id=?", (args.user,))
            store.entry(db, args.user, __import__('uuid').uuid4().hex, 'operator-revoke', 0, {'operator': args.operator, 'reason': args.reason})
        print('New Pro requests revoked; existing local files are unaffected.')
    else:
        from .network import http_client
        from .payments import SandboxPayments
        async def process():
            async with http_client() as client:
                customers = tuple(user.strip() for user in os.environ.get('GEOD_SERVICE_TEST_CUSTOMERS', '').split(',') if user.strip())
                payment = SandboxPayments(client, store, os.environ.get('GEOD_STRIPE_TEST_KEY', ''), os.environ.get('GEOD_STRIPE_TEST_WEBHOOK_SECRET', ''), customers)
                await payment.refund(args.request, args.amount_minor, args.operator, args.reason)
        asyncio.run(process())
        with store.connect() as db:
            state = db.execute('SELECT state FROM refund_requests WHERE id=?', (args.request,)).fetchone()[0]
        print('Test provider refund state:', state, '(bank settlement is separate)')


if __name__ == '__main__':
    main()
