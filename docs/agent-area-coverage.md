# Complete area acquisitions

Named administrative areas use the actual returned source boundary by default.
The model reads `boundarySource` with `geod_boundary_read`; original polygon
coordinates stay native. A rectangular search envelope is only the catalog
query. A user-selected polygon follows the same workflow. Explicit original-only
or rectangular requests retain their requested semantics.

`geod_scene_coverage` subtracts the union of validated STAC Polygon/MultiPolygon
footprints from the requested polygon or rectangle. It preserves islands and
holes, does not double-count overlap, and never substitutes a scene bbox for a
missing footprint. It returns complete, partial or unknown, a bounded gap summary
and a newest-first selection of up to 32 scenes. Reported fractions are planar
WGS84 geometric fractions, not cloud-free or valid-pixel percentages.

`geod_scene_search_more` follows a source-provided GET continuation from the same
reviewed search endpoint and retains validated scenes across pages, up to 200.
It retains the original area/date/cloud/collection validation. POST continuations
are reported as unsupported, not as an exhausted catalog. The model can query
another date interval within the agreed window. Different dates may fill gaps;
newest does not mean selecting only the newest acquisition date.

Project and download preparations, review revisions and native confirmations
reject incomplete or unknown coverage. The task card shows the exact target type
and footprint coverage. Administrative area delivery uses the project polygon
through download and the existing original-grid masked mosaic workflow. Original
files remain intermediate inputs; the goal's final raster must still be verified.

When the agreed date/cloud/product constraints cannot fill a gap, the assistant
asks through a decision card before relaxing them. Explicit automatic execution
removes redundant questions, not the coverage requirement. Source precision/year,
cloud averages, raster NoData and processing limits remain separate concerns.

The new check applies to the fixed-provider scene workflow. Custom STAC and WCS
retain their own native contracts and do not acquire a full-area claim from this
check. Older reviews/projects without stored footprints must be searched again
before a new fixed-provider area download can be confirmed. Existing files and
completed jobs are retained.
