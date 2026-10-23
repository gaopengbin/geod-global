"""Submit public sitemap URLs to Bing's documented IndexNow endpoint."""
import argparse,datetime,json,urllib.request,urllib.error,xml.etree.ElementTree as ET
from pathlib import Path
ORIGIN='https://geod-global.laogao.xyz'
def main():
 p=argparse.ArgumentParser();p.add_argument('--key-file',type=Path,required=True);p.add_argument('--report',type=Path,required=True);a=p.parse_args()
 key=a.key_file.read_text(encoding='utf-8').strip()
 assert len(key)==32 and all(c in '0123456789abcdef' for c in key)
 location=ORIGIN+'/'+key+'.txt'
 assert urllib.request.urlopen(location,timeout=20).read().decode().strip()==key,'Public ownership key differs'
 xml=ET.fromstring(urllib.request.urlopen(ORIGIN+'/sitemap.xml',timeout=20).read())
 urls=[e.text for e in xml.iter('{http://www.sitemaps.org/schemas/sitemap/0.9}loc')]
 assert len(urls)==len(set(urls))==48 and all(u.startswith(ORIGIN+'/') and '/api/' not in u for u in urls)
 body={'host':'geod-global.laogao.xyz','key':key,'keyLocation':location,'urlList':urls}
 req=urllib.request.Request('https://www.bing.com/indexnow',data=json.dumps(body).encode(),headers={'Content-Type':'application/json; charset=utf-8'},method='POST')
 try:
  with urllib.request.urlopen(req,timeout=30) as response:status=response.status;message=response.read().decode()
 except urllib.error.HTTPError as error:status=error.code;message=error.read().decode()
 receipt={'checkedAtUtc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'endpoint':'https://www.bing.com/indexnow','submittedUrls':len(urls),'status':status,'message':message,'keyFileVerified':True,'receiptMeaning':'HTTP 200 is receipt only, 202 means key validation pending. Neither establishes indexing.','publicUrls':urls}
 a.report.parent.mkdir(parents=True,exist_ok=True);a.report.write_text(json.dumps(receipt,ensure_ascii=False,indent=2),encoding='utf-8')
 print(json.dumps({k:v for k,v in receipt.items() if k!='publicUrls'},ensure_ascii=False,indent=2))
 if status not in [200,202]:raise SystemExit(1)
if __name__=='__main__':main()
