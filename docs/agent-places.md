# Global administrative lookup in the development Agent

Administrative crop requests now use `geod_boundary_read` with an actual
candidate's `boundarySource`. The returned session-scoped `boundary` hash
reference can be passed to `geod_project_plan` or `geod_clip_plan`. Original
positions stay in native storage and the reviewed project/map use the same
polygon. This read creates no imagery downloads, saved project or cropped file;
the normal native task confirmation remains required.

Census retains original shells, islands and holes instead of dropping them
after computing an envelope. Both the official `BASENAME` and qualified `NAME`
fields are queried (for example, a name ending in "city"); no city-specific
aliases are hardcoded. Source-defined municipal water areas remain present.
Natural Earth ADM0 and ADM1 reference shapes can be read offline; the native
ADM1 archive is reproduced and checked with
`python scripts/build-agent-boundary-bundle.py --check`. Only the requested
country is inflated. geoBoundaries selected shapes are cached against the
exact dataset hash and preserve its source-provided simplified precision and
license. No source is presented as a current legal boundary determination.

The Agent tries supported source geometry before asking a human to provide it.
Source failures, native geometry limits, real ambiguity and absent coverage
remain distinct; a failed request or envelope-only response never proves that
an administrative polygon does not exist. The archived genuine New York
Census response is a test fixture with provenance, not a live-fetch claim or
a production default area.

`geod_region_search` searches the bundled Natural Earth country/region layer
(242 features, 1:50m) and subdivision layer (4,596 features across 251 source
grouping codes, 1:10m v5.1.1). English, Chinese and other source-provided names
work offline. The alias file is reproducible with
`scripts/build-agent-admin-aliases.py`; repository checks pin its hash and verify
its correspondence to the original index. Natural Earth's cartographic
subdivisions do not always match a country's current ADM1 hierarchy. An explicit
country and level with no local match can use the public boundary source.

`geod_region_levels` reads a country's actual available
[geoBoundaries gbOpen levels](https://www.geoboundaries.org/api.html).
Detailed `geod_region_search` requests use that country and source level. ADM2
does not universally mean city or county. The Agent preserves administrative
type, translates to an established local or English name when needed, and asks
only about genuinely ambiguous places. Humans need not supply ISO codes, level
numbers or coordinates. A separate Photon gazetteer can resolve cities absent
from administrative datasets. Explicit US city lookups can use public Census
city geometry; that adapter has no New York aliases or special routing.

Detailed lookup validates source-provided simplified Polygon/MultiPolygon
geometries and derives WGS84 envelopes from actual coordinates. Results retain
source identity, source year, original license and attribution, geometry byte
hash and checked time. Source grouping codes do not establish political status.
An envelope is a search rectangle, not the polygon, a legal boundary decision
or proof of complete imagery coverage. Intermediate parents are not inferred
from bounding-box overlap. Cropping uses the separately read actual geometry.

Invalid catalog rows are reported as `coverageIssues`; other valid levels remain
queryable. Actual `geometryUnits` and advertised `declaredUnits` are independent.
`unitCountMatchesMetadata=false` reports source count discrepancies without
inventing missing units or disabling valid features. Source label whitespace
is normalized while the exact geometry hash is retained.

Metadata and derived country-level indexes persist for seven days. Each index
page has a hash and a manifest is published only after all pages are saved.
Restarted reads validate pages, IDs, counts and source identity. Corrupt or
partial caches cannot supply coordinates. Only fixed gbOpen API addresses and
commit-pinned files in `wmgeolab/geoBoundaries` are accepted. All requests honor
the native proxy policy. Remote lookup has a 45-second deadline and a
64 MiB / 100,000-unit dataset limit; larger catalog levels are flagged explicitly.
There is no public Nominatim integration, bulk autocomplete or arbitrary model URL.

Coverage depends on each country's available level, source year and names.
This is not a claim that every village or current administrative change is
available. Empty matches, absent/inconsistent coverage, network failures,
invalid geometry and service refusals remain distinct. A resolved envelope can
enter the normal imagery search and native review workflow. Lookup submits no
download and does not authorize execution.

Native tests cover multiple continents, multilingual names, diacritics,
country/state ambiguity, territory ISO collisions, invalid source identities,
schema discrepancies, restart and corrupted caches. Opt-in live source and
model tests use isolated stores, with no desktop manipulation:

```text
cargo test --locked -p geod-runtime --lib agent_actions::regions
GEOD_REGIONS_QA=<isolated-dir> cargo test --locked -p geod-runtime --lib live_global_administrative_lookup_and_persistent_cache -- --ignored --nocapture
python scripts/verify-agent-stac-model.py --test-binary <native-test-exe> --scenario regions
```

The Windows development app can inherit a packaged launcher's merged AppData
view without a package identity of its own. Managed Agent record directories
accept only the exact same-profile LocalCache correspondence proven by the OS
handle of an exclusively created, random managed file. The temporary file is
removed on every return path. This does not depend on a launcher's process tree
remaining alive. Symlinks, reparse points, changed roots and a package/path that
does not match that fresh write witness are still rejected. The explicit native System-route probe can validate a stopped
app's actual runtime with a separate `GEOD_AGENT_SYSTEM_QA` receipt directory and
`GEOD_AGENT_SYSTEM_CORE` runtime directory, then reopen its place/admin caches.
`scripts/verify-agent-detached-profile.py` launches that probe behind a gate and
releases it only after the launch process exits, reproducing the development
app's detached lifetime rather than only testing a short-lived command child.

Once a turn requests a city lookup, imagery search must use a successful native
city candidate's extent. A failed lookup cannot be replaced by a same-named
state or unrelated current map. A native record-storage refusal ends geographic
retries in that turn. History persists only fixed failure codes, localized in
the tool log; raw provider errors, file paths and credentials are not displayed.
Review-only model acceptance can use an explicitly stopped actual runtime via
`--scenario place --source-system --place-core <runtime-dir>`: conversations
remain in QA storage, existing jobs and network settings are preserved, and no
download is submitted. This differs from earlier isolated-store evidence.

An old unanswered fallback card asserting that a polygon is missing can become
`superseded` when an actual native polygon read resolves the same crop request
and verified extent. Legacy cards require their four explicit envelope
coordinates to match; new fallback cards can retain the native city scope.
No human answer is invented. Answered cards, genuine geometry preferences,
mixed questions, different places and unrelated pending choices remain intact.
The resolved card records the native boundary hash and hides the obsolete upload
prompt. A rectangular plan still cannot satisfy a polygon crop, and final native
confirmation remains required. The original failed conversation was tested in
an isolated copy with its actual model connection; it produced a pending polygon
project, with zero download jobs.

See [verification](../prototype/qa/agent-global-regions-verification.json).
