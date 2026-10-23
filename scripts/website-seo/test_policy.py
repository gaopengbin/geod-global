"""Regression tests for the public website release boundary (stdlib only)."""
import json,tempfile,unittest
from pathlib import Path
from seo_policy import prepare,check
class ReleaseGate(unittest.TestCase):
 def setUp(self):
  self.tmp=tempfile.TemporaryDirectory();self.site=Path(self.tmp.name)
  locales={'en':'','zh-CN':'zh','es':'es','fr':'fr','de':'de','pt-BR':'pt-br','ja':'ja','ko':'ko'}
  routes=['index.html','agent.html','support.html','workflows/elevation.html','workflows/landsat.html','workflows/sentinel-2.html','early-access.html','privacy.html','auth-complete.html']
  for locale,prefix in locales.items():
   for route in routes:
    alts=''.join(f'<link rel="alternate" hreflang="{l}" href="/{s+"/" if s else ""}{route}">' for l,s in locales.items())+f'<link rel="alternate" hreflang="x-default" href="/{route}">'
    p=self.site/prefix/route;p.parent.mkdir(parents=True,exist_ok=True)
    p.write_text(f'<html lang="{locale}"><head><title>Test product</title><meta name="description" content="Test"><meta name="robots" content="noindex, nofollow">{alts}</head><body></body></html>',encoding='utf-8')
  (self.site/'language-data.js').write_text('window.GeoDSiteCopy='+json.dumps({l:['test']*415 for l in locales})+';\nwindow.GeoDSiteLocales='+json.dumps(list(locales))+';\n')
  prepare(self.site)
 def tearDown(self):self.tmp.cleanup()
 def change(self,name,old,new):
  p=self.site/name;p.write_text(p.read_text(encoding='utf-8').replace(old,new),encoding='utf-8')
 def test_release_and_idempotence(self):
  before={p:p.read_bytes() for p in self.site.rglob('*') if p.is_file()};prepare(self.site)
  self.assertEqual(before,{p:p.read_bytes() for p in self.site.rglob('*') if p.is_file()})
  self.assertEqual(check(self.site)['publicPages'],48)
 def test_noindex_rejected(self):
  self.change('zh/index.html','index, follow','noindex, nofollow')
  with self.assertRaises(ValueError):check(self.site)
 def test_private_page_stays_private(self):
  self.change('zh/privacy.html','noindex, nofollow','index, follow')
  with self.assertRaises(ValueError):check(self.site)
 def test_canonical_required(self):
  self.change('index.html','rel="canonical"','rel="wrong"')
  with self.assertRaises(ValueError):check(self.site)
 def test_sitemap_required(self):
  self.change('sitemap.xml','/zh/index.html','/en/index.html')
  with self.assertRaises(ValueError):check(self.site)
 def test_alternate_must_resolve(self):
  self.change('index.html','hreflang="zh-CN" href="https://geod-global.laogao.xyz/zh/index.html"','hreflang="zh-CN" href="https://geod-global.laogao.xyz/broken.html"')
  with self.assertRaises(ValueError):check(self.site)
if __name__=='__main__':unittest.main()
