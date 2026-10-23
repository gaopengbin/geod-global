# Website SEO release gate

The Global website is maintained independently in the `geod-global-launch` workspace. The deployed domain is https://geod-global.laogao.xyz/ and English lives at `/`, Chinese at `/zh/`. Legacy `/en/*` URLs permanently redirect to `/*`, retaining query strings. New Chinese announcements should link directly to `/zh/index.html`.

## Required release checks

- Preview builds remain `noindex, nofollow`. Use `python tools/build-site.py --public` for public output, or `tools/prepare_release.py` for the full public payload. Both apply `tools/seo_policy.py`.
- Run `python tools/check-site.py --public --site <candidate/site>` before deployment. Public mode fails on any index-blocking public page, incorrect canonical, broken/incomplete locale alternates, sitemap drift, or accidentally indexed account/privacy pages.
- There are 48 indexable public pages and 24 excluded account/privacy pages across eight languages. Change this explicit allowlist and its tests together when adding routes.
- Upload verified local artifacts through pinned-host SSH. On the server, validate the complete candidate before atomically switching `/srv/laogao/current/geod-global`; retain the previous immutable release for rollback. Do not upload preview HTML directly to the current release.
- The service additionally runs `/srv/laogao/control/geod-global-seo/seo_policy.py` in `ExecStartPre`, configured in `/etc/systemd/system/geod-global.service.d/seo-gate.conf`. An invalid public release cannot start. Roll back the current symlink if activation fails; do not disable the gate to publish a preview build.
- The checked policy is versioned here in `scripts/website-seo/seo_policy.py`. Keep the website workspace's policy identical when changing it. GitHub workflow `website-seo.yml` runs regression tests without installing dependencies or storing build artifacts.
- Read back all sitemap pages after release, verify canonical and robots metadata, check `/en/index.html` and `/en/workflows/sentinel-2.html` redirect to the equivalent current pages, and verify the current CDN setup/portable links, demo, and API health.

## 2026-10-23 correction

### Search engine submissions

- Bing: 48 sitemap URLs were submitted to `https://www.bing.com/indexnow` on 2026-10-23. The initial HTTP 202 was followed by HTTP 200 after the public ownership key had been validated. This confirms receipt, not indexing. The public root `.txt` proof must remain in the website source and subsequent static releases. `scripts/website-seo/submit_indexnow.py` reads a specified public key file, verifies it through HTTPS, and submits only the 48 public sitemap URLs; it writes a receipt and fails on errors.
- Baidu: the Global HTTPS subdomain was added and ownership verified using the console-provided public HTML verification tag. Public builds now reinsert that tag in `seo_policy.prepare`, and the regression suite checks it. The website attributes include the existing public parent-domain ICP filing and responsive/adaptive website type.
- Baidu's console requires linking an existing Baijiahao subject before resources can enter search, and warns that linking cannot be undone for seven days. Do not silently link an unconfirmed subject. Record account eligibility, remaining quota, accepted submissions, crawl/index state separately; a verified site is not a successful URL submission. Never store the private Baidu push token in the repository or a public report.
- Current submission evidence is stored in the website workspace under `.verification/seo-20261023/`. Re-check platform status when reporting progress; no live user counts or private credentials belong in this file.

The production site had inherited preview `noindex, nofollow` metadata and lacked canonical tags. The static overlay fixes those settings while preserving current download links, videos, provider descriptions, account services and usage collection. Home title/description now explain satellite imagery and geospatial data downloads in English and Chinese. Search-engine recrawling and actual inclusion are separate from passing these technical checks; never report inclusion merely because HTTP and metadata checks passed.
