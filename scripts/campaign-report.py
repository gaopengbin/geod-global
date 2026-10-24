"""Read-only, QA-excluded Landsat campaign observations. Never joins browser to desktop."""
import argparse,json,sqlite3
from collections import Counter
from pathlib import Path

CAMPAIGN='landsat_batch_20261023'
SOURCES=('zhihu','bilibili')

def summarize(rows,excluded):
    campaign=[e for e in rows if e['environment']=='production' and e['fields'].get('acquisitionCampaign')==CAMPAIGN]
    external=[e for e in campaign if e['installationId'] not in excluded]
    report={'campaign':CAMPAIGN,'scope':'observed-anonymous-installations-not-people',
            'internalEventsExcluded':len(campaign)-len(external),'channels':{}}
    for source in SOURCES:
        events=[e for e in external if e['fields'].get('acquisitionSource')==source]
        web=[e for e in events if e['platform']=='browser' and e['fields'].get('acquisitionKind')=='website']
        desktop=[e for e in events if e['platform']=='desktop']
        count=Counter(e['name'] for e in desktop)
        installations=lambda name:len({e['installationId'] for e in desktop if e['name']==name})
        report['channels'][source]={
            'tutorialViews':sum(e['name']=='tutorial_viewed' for e in web),
            'observedBrowserIdentifiers':len({e['installationId'] for e in web}),
            'downloadClicks':sum(e['name']=='tutorial_download_clicked' for e in web),
            'observedDesktopInstallations':len({e['installationId'] for e in desktop}),
            'newInstallationFirstRuns':len({e['installationId'] for e in desktop if e['name']=='app_first_run' and e['fields'].get('acquisitionKind')=='new_installation'}),
            'existingInstallations':len({e['installationId'] for e in desktop if e['fields'].get('acquisitionKind')=='existing_installation'}),
            'installationsWithSuccessfulSearch':installations('search_completed'),
            'completedDownloadFiles':count['download_completed'],
            'validatedFiles':count['artifact_validated'],
            'installationsWithValidatedFiles':installations('artifact_validated'),
            'installationsOpeningResults':installations('artifact_opened'),
            'failureEvents':sum(n for name,n in count.items() if name.endswith('_failed')),
        }
    report['limits']='Independent website and desktop identifiers; no person-level join or conversion rate. Clicks do not prove transfer or install. Missing reports remain unknown.'
    return report

def main():
    p=argparse.ArgumentParser();p.add_argument('--db',type=Path,required=True);p.add_argument('--registry-dir',type=Path,required=True);p.add_argument('--since',default='2026-10-23T00:00:00.000Z');a=p.parse_args()
    if not a.db.is_file():raise SystemExit('Usage database unavailable; observations unknown.')
    registries=list(a.registry_dir.glob('qa-*.json'))
    if not registries:raise SystemExit('QA registries unavailable; refusing unfiltered external-user counts.')
    excluded=set()
    for path in registries:
        value=json.loads(path.read_text());excluded.update(value.get('installationIds',[]))
    db=sqlite3.connect(a.db.resolve().as_uri()+'?mode=ro',uri=True);db.execute('PRAGMA query_only=ON')
    rows=[json.loads(row[0]) for row in db.execute('SELECT payload FROM events WHERE at>=?',(a.since,))]
    report=summarize(rows,excluded);report['sinceUtc']=a.since;report['qaRegistries']=len(registries)
    print(json.dumps(report,ensure_ascii=False,indent=2))
if __name__=='__main__':main()
