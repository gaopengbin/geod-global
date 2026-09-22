# Third-party assets and data

- **Inter typeface:** SIL Open Font License 1.1; full notice is included in
  `fonts/LICENSE-Inter.txt`. Embedded font files originate from the repository's
  `prototype/public/fonts` directory.
- **SCL display legend:** class meanings and display colors follow the
  [Sentinel Hub SCL legend](https://custom-scripts.sentinel-hub.com/custom-scripts/sentinel-2/scene-classification/)
  by Sentinel Hub, published under CC BY-SA 4.0. The application independently
  implements the class/color mapping. The license is available at
  [Creative Commons](https://creativecommons.org/licenses/by-sa/4.0/legalcode.en).
- **Sample scene thumbnails and catalog records:** Copernicus Sentinel-2 data,
  exposed by Earth Search / Element 84. Per-file URLs, hashes and attribution
  are retained in the embedded sample manifest. Data use follows the
  [Copernicus Sentinel legal notice](https://sentinels.copernicus.eu/documents/247904/690755/Sentinel_Data_Legal_Notice).
- **Live downloaded raster data:** not shipped inside this package. Provider
  attribution, original asset URL and source checksum accompany each local job
  and generated crop sidecar.

`inventory.json` enumerates the locked Rust graph and installed npm packages,
including build dependencies and platform-optional packages. This is deliberately
broader than a claim that each package is linked into the binaries. Exact MPL
crate source is included under `source/`; the remaining source is identified by
package version and repository in the inventory. License files missing from a
published crate are fetched from its declared official GitHub repository at the
exact commit recorded in that crate's `.cargo_vcs_info.json`. Where proj4rs or
selectors also omit a standalone file in that commit, the package includes the
unmodified standard Apache-2.0 or MPL-2.0 terms, published license declaration and
complete original crate source with its copyright headers. Lerc similarly includes
its original JavaScript copyright/license header and Apache-2.0 terms. Platform
binding packages inherit the texts from their matching same-version parent package;
these origins are distinguished in the inventory.
