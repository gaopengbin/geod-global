"""Replay an explicitly supplied public search with independent HTTP requests.

Compares complete source pages with a previous native acquisition receipt, then
checks every pinned snapshot's exact page method/body. This contacts the chosen
public origin; it does not download original assets or claim account acceptance.
"""
import argparse
from datetime import datetime, timezone
import hashlib
import ipaddress
import json
from pathlib import Path
import urllib.parse
import urllib.request


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        return None


def digest(raw):
    return hashlib.sha256(raw).hexdigest()


def read(path):
    return json.loads(path.read_bytes())


def save(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2, allow_nan=False)+'\n', encoding='utf-8')


def public(url, origin):
    parts = urllib.parse.urlsplit(url)
    assert parts.scheme == 'https' and not parts.username and not parts.password and not parts.fragment
    assert parts.netloc == urllib.parse.urlsplit(origin).netloc
    assert parts.hostname and parts.hostname not in ('localhost',) and not parts.hostname.endswith(('.local','.internal'))
    try:
        assert ipaddress.ip_address(parts.hostname).is_global
    except ValueError:
        pass
    assert not any(key.lower() in ('access_token','api_key','sig','signature','password','authorization') or key.lower().startswith(('x-amz-','x-goog-')) for key, _ in urllib.parse.parse_qsl(parts.query))


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source-report', type=Path, required=True)
    parser.add_argument('--data-dir', type=Path, required=True)
    parser.add_argument('--request', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args=parser.parse_args()
    source=read(args.source_report)
    assert source['status']=='passed' and source['kind']=='api' and source['searchComplete']
    assert not args.output.exists() and not args.output.resolve().is_relative_to(args.data_dir.resolve())
    args.output.mkdir(parents=True)
    original=read(args.request)
    current=json.loads(json.dumps(original))
    native_pages=source['searchPages']
    assert 1 <= len(native_pages) <= 20
    opener=urllib.request.build_opener(urllib.request.ProxyHandler({}), NoRedirect())
    report={'status':'running','startedAt':datetime.now(timezone.utc).isoformat(),'sourceReportSha256':digest(args.source_report.read_bytes()),
        'scope':'Independent public metadata requests; not an original-asset download or an account test','pages':[],'snapshots':[]}
    save(args.output/'report.json',report)
    try:
        seen=set()
        all_items=[]
        for index, native in enumerate(native_pages):
            assert current is not None
            public(current['url'],original['url'])
            assert current['method'] in ('GET','POST')
            assert 'body' not in current or isinstance(current['body'],dict) and current['method']=='POST'
            encoded=None if current['method']=='GET' else json.dumps(current.get('body',{}),ensure_ascii=False).encode()
            assert encoded is None or len(encoded)<=16384
            request=urllib.request.Request(current['url'],data=encoded,method=current['method'],headers={'Accept':'application/geo+json, application/json','Content-Type':'application/json'})
            with opener.open(request,timeout=45) as response:
                assert response.status==200
                raw=response.read(8*1024*1024+1)
                assert 0<len(raw)<=8*1024*1024
            independent=json.loads(raw)
            expected=read(args.source_report.parent/native['file'])
            assert digest((args.source_report.parent/native['file']).read_bytes())==native['sha256']
            assert independent==expected, f'Independent public page {index+1} changed'
            name=f'page-{index+1:02}.json'; (args.output/name).write_bytes(raw)
            identities=[(value.get('collection'),value['id']) for value in independent['features']]
            assert not seen.intersection(identities); seen.update(identities); all_items.extend(identities)
            report['pages'].append({'file':name,'request':current,'sha256':digest(raw),'bytes':len(raw),'items':len(identities),
                'nativeDocumentSha256':native['sha256'],'allOriginalFieldsEqual':True})
            links=[value for value in independent['links'] if value['rel']=='next']
            assert len(links)<=1
            following=None
            if links:
                link=links[0]
                assert not link.get('headers')
                following={'url':urllib.parse.urljoin(current['url'],link['href']),'method':link.get('method','GET')}
                if 'body' in link: following['body']=link['body']
                if link.get('merge',False):
                    assert following['method']==original['method']=='POST' and isinstance(link.get('body'),dict)
                    following['body']={**original.get('body',{}),**link['body']}
            current=following
        assert current is None and len(all_items)==source['searchItems']
        for snapshot in source['snapshots']:
            path=args.data_dir/'stac'/f"snapshot-{snapshot['snapshotId']}.json"
            assert digest(path.read_bytes())==snapshot['snapshotId']
            record=read(path)
            matches=[page for page in report['pages'] if page['nativeDocumentSha256']==record['documentSha256']]
            assert len(matches)==1
            assert record['documentRequest']==matches[0]['request']
            assert record['documentUrl']==matches[0]['request']['url']
            report['snapshots'].append({'id':snapshot['snapshotId'],'exactPageRequestEqual':True})
        report.update(status='passed',totalItems=len(all_items),finishedAt=datetime.now(timezone.utc).isoformat())
    except Exception as error:
        report.update(status='failed',error=str(error))
        raise
    finally:
        save(args.output/'report.json',report)
    print(json.dumps({'status':report['status'],'pages':len(report['pages']),'items':report['totalItems'],'snapshots':len(report['snapshots'])}))


if __name__=='__main__': main()
