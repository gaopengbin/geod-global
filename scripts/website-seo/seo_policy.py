"""Prepare and verify public metadata. Preview sources remain non-indexable."""
import argparse, hashlib, json, re
from html import escape, unescape
from html.parser import HTMLParser
from pathlib import Path
from urllib.parse import urlsplit
import xml.etree.ElementTree as ET

ORIGIN='https://geod-global.laogao.xyz'
HOMES={
 'en':('GeoD Global — Satellite imagery and geospatial data downloads','Discover and download satellite imagery, elevation, land cover and other geospatial data. Search by area, preview multiple layers and manage local results on Windows.'),
 'zh-CN':('GeoD Global — 遥感影像与地理空间数据下载工具','聚合 Sentinel、Landsat、DEM、土地覆盖、地理空间数据云等多类型数据渠道，支持区域检索、多选地图预览、数据下载与本地成果管理，可连接自己的 AI 模型。')}

def public(route):
 return Path(route).name in ('index.html','agent.html','support.html') or '/workflows/' in '/'+route

class Head(HTMLParser):
 def __init__(self,text):
  super().__init__();self.robots=[];self.canon=[];self.alts={};self.title='';self.in_title=False;self.feed(text)
 def handle_starttag(self,tag,attrs):
  a=dict(attrs)
  if tag=='meta' and a.get('name','').lower() in ('robots','googlebot','baiduspider'):self.robots.append(a.get('content','').lower())
  if tag=='link' and a.get('rel')=='canonical':self.canon.append(a.get('href'))
  if tag=='link' and a.get('rel')=='alternate' and 'hreflang' in a:self.alts[a['hreflang']]=a.get('href')
  if tag=='title':self.in_title=True
 def handle_endtag(self,tag):
  if tag=='title':self.in_title=False
 def handle_data(self,data):
  if self.in_title:self.title+=data

def prepare(site,origin=ORIGIN):
 assert origin==ORIGIN
 lang=site/'language-data.js';raw=lang.read_text(encoding='utf-8')
 copies,tail=raw.split('window.GeoDSiteCopy=',1)[1].split(';\nwindow.GeoDSiteLocales=',1)
 copies=json.loads(copies)
 for locale,(title,desc) in HOMES.items():copies[locale][52]=title;copies[locale][414]=desc
 lang.write_text('window.GeoDSiteCopy='+json.dumps(copies,ensure_ascii=False)+';\nwindow.GeoDSiteLocales='+tail,encoding='utf-8')
 rev=hashlib.sha256(lang.read_bytes()).hexdigest()[:12];routes=[]
 for path in sorted(site.rglob('*.html')):
  route=path.relative_to(site).as_posix();t=path.read_text(encoding='utf-8')
  locale=re.search(r'<html[^>]*lang="([^"]+)"',t)[1]
  if path.name=='index.html' and locale in HOMES:
   title,desc=HOMES[locale]
   t=re.sub(r'(<title\b[^>]*>).*?(</title>)',lambda m:m[1]+escape(title)+m[2],t,flags=re.S)
   t=re.sub(r'(<meta name="description" content=")[^"]*',lambda m:m[1]+escape(desc,quote=True),t)
  t=re.sub(r'<meta\b(?=[^>]*name="robots")[^>]*>','',t)
  t=re.sub(r'<link\b(?=[^>]*rel="canonical")[^>]*>','',t)
  t=re.sub(r'<meta\b(?=[^>]*(?:property|name)="(?:og:[^"]+|twitter:[^"]+)")[^>]*>','',t)
  t=re.sub(r'<script type="application/ld\+json">.*?</script>','',t,flags=re.S)
  t=re.sub(r'(<link rel="alternate"[^>]*href=")(?:https://geod-global\.laogao\.xyz)?(/[^" ]*)',lambda m:m[1]+origin+('/' if m[2]=='/index.html' else m[2]),t)
  t=re.sub(r'(/language-data\.js\?v=)[0-9a-f]+',lambda m:m[1]+rev,t)
  url=origin+('/' if route=='index.html' else '/'+route)
  is_public=public(route)
  if is_public:routes.append(url)
  head=Head(t);desc=re.search(r'<meta name="description" content="([^"]*)"',t)
  meta='<meta name="robots" content="'+('index, follow' if is_public else 'noindex, nofollow')+'">'
  meta+='<link rel="canonical" href="'+url+'">'
  if is_public:
   for key,value in [('og:type','website'),('og:site_name','GeoD Global'),('og:title',head.title),('og:description',unescape(desc[1]) if desc else head.title),('og:url',url),('og:image',origin+'/assets/sentinel2-workspace-en.jpg')]:meta+=f'<meta property="{key}" content="{escape(value,quote=True)}">'
   meta+='<meta name="twitter:card" content="summary_large_image">'
  t=t.replace('</head>',meta+'</head>');path.write_text(t,encoding='utf-8')
 sitemap=ET.Element('urlset',xmlns='http://www.sitemaps.org/schemas/sitemap/0.9')
 for url in routes:ET.SubElement(ET.SubElement(sitemap,'url'),'loc').text=url
 ET.ElementTree(sitemap).write(site/'sitemap.xml',encoding='utf-8',xml_declaration=True)
 (site/'robots.txt').write_text('User-agent: *\nDisallow: /api/\nSitemap: '+origin+'/sitemap.xml\n',encoding='utf-8')
 return check(site,origin)

def check(site,origin=ORIGIN):
 errors=[];public_count=private_count=0
 ns='{http://www.sitemaps.org/schemas/sitemap/0.9}'
 try:listed={n.text for n in ET.parse(site/'sitemap.xml').iter(ns+'loc')}
 except Exception as e:listed=set();errors.append('Missing or invalid sitemap: '+str(e))
 expected=set()
 for p in sorted(site.rglob('*.html')):
  route=p.relative_to(site).as_posix();head=Head(p.read_text(encoding='utf-8'));url=origin+('/' if route=='index.html' else '/'+route)
  if not head.title.strip():errors.append(route+': missing title')
  if head.canon!=[url]:errors.append(route+': canonical missing, duplicated or incorrect')
  if public(route):
   public_count+=1;expected.add(url)
   if not head.robots or any('noindex' in r or 'nofollow' in r or r.strip()=='none' for r in head.robots):errors.append(route+': public page blocks indexing/following')
   if len(head.alts)!=9:errors.append(route+': incomplete locale alternates')
   for locale,href in head.alts.items():
    if not href.startswith(origin+'/'):errors.append(route+': non-absolute alternate');continue
    target=site/urlsplit(href).path.lstrip('/')
    if href==origin+'/':target=site/'index.html'
    if not target.is_file():errors.append(route+': broken alternate '+locale)
  else:
   private_count+=1
   if not any('noindex' in r for r in head.robots):errors.append(route+': private page is indexable')
 if listed!=expected:errors.append('Sitemap differs from public route allowlist')
 if public_count!=48 or private_count!=24:errors.append('Unexpected locale coverage')
 robots=(site/'robots.txt').read_text(encoding='utf-8') if (site/'robots.txt').exists() else ''
 if 'Disallow: /\n' in robots or origin+'/sitemap.xml' not in robots:errors.append('Invalid public robots.txt')
 if errors:raise ValueError('\n'.join(errors))
 return {'publicPages':public_count,'privatePages':private_count,'errors':[]}

if __name__=='__main__':
 p=argparse.ArgumentParser();p.add_argument('--site',type=Path,required=True);p.add_argument('--prepare',action='store_true');a=p.parse_args()
 print(json.dumps((prepare if a.prepare else check)(a.site.resolve()),ensure_ascii=False))
