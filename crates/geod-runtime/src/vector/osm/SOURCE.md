# OSM polygon classification data

- Upstream: [tyrasd/osm-polygon-features](https://github.com/tyrasd/osm-polygon-features).
- Version: 0.9.2; author: Martin Raifer.
- Published package Git commit: `069a46416629ddbe84390c7a3ccc600fe73ee706` (the `gitHead` from the [npm version metadata](https://registry.npmjs.org/osm-polygon-features/0.9.2)).
- Pinned source: [polygon-features.json](https://github.com/tyrasd/osm-polygon-features/blob/069a46416629ddbe84390c7a3ccc600fe73ee706/polygon-features.json).
- License: CC0-1.0, reproduced unchanged in `POLYGON-FEATURES-LICENSE`.
- Retrieved 2026-10-02 from the [published 0.9.2 package](https://registry.npmjs.org/osm-polygon-features/-/osm-polygon-features-0.9.2.tgz). Package SHA-1 `20ae41130c486e49a3b2a3c2b58a1419c4986778` matched the registry metadata.
- Unmodified JSON SHA-256: `cf81018ba820557c59c2c27de7ea6b314009abed040cd11249e94ed1a3a10583`.
- Unmodified license SHA-256: `36ffd9dc085d529a7e60e1276d73ae5a030b020313e6c5408593a6ae2af39673`.

The Rust converter embeds only the JSON data. It does not run upstream JavaScript or add an npm dependency. GeoD requires a closed way before consulting these rules, gives `area=no` precedence, and preserves explicit `area=yes`. Geometry and tags remain source data; these rules determine the derived GeoJSON geometry type. Previously registered extracts retain the legacy converter to preserve their recorded GeoJSON checksums.
