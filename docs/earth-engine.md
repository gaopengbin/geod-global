# Google Earth Engine export integration

## Implemented scope

The Explore source add menu and data-source directory open **Google Earth Engine exports**. The user runs a bounded Landsat 8/9 band export in their own GEE editor/project, then pastes the generated `projects/.../thumbnails/...:getPixels` URL. GeoD downloads and fully decodes the GeoTIFF, checks its georeference, saves SHA-256 and project identity, and provides a first-band grayscale preview and a save-file action. Saved exports can be inspected again without Google access.

This is a **generated subset export**, not a byte-identical original scene, a general GEE catalog connector, automatic Google OAuth, or an automatic alternative to Planetary Computer. The script reports LANDSAT_PRODUCT_ID so the user can compare processing identity, but the downloaded URL alone cannot verify dataset identity or original-grid alignment. The software does not infer these from the export name. GEE integer NoData declarations such as `0.0` are accepted exactly; fractional integer NoData and overflow remain rejected.

The script adds a unique `workload_tag`, uses the project selected by the user in Google, and estimates the selected region conservatively before generating a 30 m single-band request. It does not pin `geod-global-dev`. No Google credentials, temporary URLs, or embedded authorization tokens are persisted. The native receiver accepts only the reviewed HTTPS GEE getPixels endpoint, with no redirects, automatic retries, query credentials or custom hosts. Downloads are capped at 32 MiB / 180 seconds, decoded samples are bounded, and only verified results are committed to the managed `earth-engine/<UUID>/` directory.

## Usage evidence and Pro boundary — 2026-10-11

The user's Cloud Console screenshot for `geod-global-dev` shows **400 noncommercial EECU-seconds used this month**, against **540,000** allowed, displayed as **0.07%**. This establishes nonzero **project-level** usage. It does not establish that one 896 KiB download consumed 400 seconds. An earlier screenshot showed zero; reporting lag and other editor operations prevent per-file attribution from those two screenshots.

The export API computes an image; successful computations are reported to Cloud Monitoring. Use **Completed EECU-seconds**, grouped by `workload_tag` and `compute_type`, with alignment and grouping set to **Sum**, to measure tagged runs. Network elapsed seconds and downloaded bytes are not EECU measurements. GeoD records `eecuSeconds: null` and `usageStatus: not_measured` until an authenticated monitoring integration exists; it does not fabricate zero consumption or convert elapsed time to compute cost.

Product decision:

- **Bring your own project:** current local connector; consumes the project encoded in the user's GEE export link. Do not charge official compute credits merely for downloading through this local tool.
- **Official hosted GEE:** a future Pro candidate because it consumes operator-owned compute/quota. Not enabled, and no shared developer project credentials are distributed to clients.
- Before launching the hosted service, use a Google-approved commercial configuration appropriate to the actual service. Noncommercial free allocation does not become commercially reusable merely by adding a Pro switch.
- Hosted enforcement must be server-side: authenticated entitlement, per-user concurrency and workload caps, reservation/idempotency, tagged job execution, authoritative usage reconciliation and cancellation/error handling. The open-source client is not the enforcement boundary.
- No pricing per file, per GB, or per EECU is claimed from the single aggregate screenshot. Measure representative cold and repeated exports first. Monitoring unavailable must remain unknown; no production hosted billing begins on guessed usage.

## Official references

- [Download URL size/grid limits](https://developers.google.com/earth-engine/apidocs/ee-image-getdownloadurl)
- [getPixels computes an image](https://developers.google.com/earth-engine/reference/rest/v1/projects.thumbnails/getPixels)
- [Monitoring usage and workload tags](https://developers.google.com/earth-engine/guides/monitoring_usage)
- [Noncommercial monthly tiers](https://developers.google.com/earth-engine/guides/noncommercial_tiers)
- [Noncommercial eligibility and commercial setup](https://developers.google.com/earth-engine/guides/access)
- [Commercial pricing](https://cloud.google.com/earth-engine/pricing)

## Validation

Validation distinguishes real HTTP downloads, full TIFF decoding, hash verification and offline restoration from mocked UI tests. Browser/native visual acceptance must be recorded separately; a build is not a visual check. Live test links and user credentials must not be put in committed tests or documentation.
