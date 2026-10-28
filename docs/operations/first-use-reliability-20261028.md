# First-use reliability work — 2026-10-28

This work prioritizes actual task completion over expanding promotion. It is a local repair and investigation record, not an announcement that the historical customer failures are resolved or a new installer has shipped.

## What was changed

Reviewed built-in public downloads now have one 45-second budget from starting the source request to receiving the first response-body bytes. It begins after queue admission and after finite/generated adapters, spans incomplete response headers and a range restart, and remains cancellable. Once source bytes arrive, the existing inactivity guard applies; this is not a total-duration cap for large files. The existing maximum of two automatic retries and checkpoint/hash validation remain intact. Custom STAC, WCS, protected and generated workflows retain their own contracts.

Retries waiting to reacquire a worker slot now show queued status instead of retaining the previous attempt's running status. Partial-byte records remain intact; waiting is no longer presented as an active zero-byte transfer.

Public-product client failures now distinguish service requests, JSON decoding and response-contract validation. Available HTTP status is retained even if a response body cannot be decoded. The diagnostic fields reuse the existing allowlist; raw messages, URLs, coordinates and keys are not uploaded. This does not recover missing details from older clients.

## Regression verification

- Real loopback HTTP faults cover incomplete headers, complete headers followed by no body, cancellation, queue waiting, and a healthy body that finishes after the first-byte budget. The tests use a shortened budget through a private implementation parameter; production uses 45 seconds. The queue remains queued and does not spend that budget.
- Runtime tests: 928 passed, 46 explicitly ignored. Runtime Clippy passed with warnings denied. Fault fixtures verify transport behavior, not imagery science.
- Public-product client boundary tests cover network errors, malformed JSON with HTTP 503, HTTP 403, native rejection and malformed returned catalog contracts. Sensitive text remains outside the emitted diagnostic fields.
- Complete frontend verification and production build passed. After starting the required local UI service, real Workspace views rendered previous Landsat RGB, a clipped DEM and Sentinel SCL results and read their original pixels. Production telemetry was blocked during these UI checks. An earlier attempt without that local service running was an incomplete test and is not counted as a product pass or provider failure.
- A new 95,194,940-byte Landsat red-band download was interrupted once by an isolated TLS-preserving proxy. The modified runtime resumed from 5,177,344 bytes, recorded one retry, and passed whole-file SHA-256 verification (`8a06e26a0e061038a9c7046347d611bdbc2503e4ff165f87f3b37f46f858b6c2`). This is separate from the transport fixtures.

## CBERS live investigation

The original 0.2.1 runtime and the released 0.2.3 runtime each resolved and searched the official INPE WPM catalog successfully for the same independent test area and month, returning four scenes. The 26 historical sub-second failures were not reproduced. Current successful requests therefore do not establish their cause or prove that those users' failed requests are fixed.

The modified development runtime then completed a real original-band delivery:

- Product: CBERS-4A WPM L4 multispectral original DN, 8 m; BAND1.
- Original size: 138,963,475 bytes.
- SHA-256: `6e8fd3284b2845d62c586650fd3b5e24a3893fb7787c401033e503bd6534a967`, independently read from the downloaded file and matching the publisher's multihash.
- Grid: 14,265 × 14,634, EPSG:32722.
- Native local preview decoded. The original center-pixel value, 206, matched an independent raster read. No reflectance conversion or pansharpening was applied.

This checks native catalog → project selection → original download → local preview and pixel reading. It does not claim an entire desktop UI acceptance or explain the historical users' unknown errors. All requests used isolated local QA data and emitted no production usage events.

## Remaining acceptance and limits

The historical zero-byte task included queue time in its elapsed bucket and reported only a coarse network error. The new first-data deadline is a tested bound on waiting, not proof of that event's root cause. NAIP empty results likewise cannot be attributed to a particular area or date because neither is collected. Do not count empty searches as transport failures, invent missing diagnoses, or silently substitute another product.

Keep the public 0.2.3 installer unchanged until a subsequent package is built and accepted. Historical failures and any local successful reproductions remain distinct evidence.
