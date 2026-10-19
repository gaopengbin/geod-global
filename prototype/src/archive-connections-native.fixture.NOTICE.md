This fixture was captured on 2026-10-14 from the running GeoD Global native
service through normal `POST /datasets/connect` and immutable
`GET /stac/snapshots/{id}` responses. It contains source metadata, reviewed
publisher original ZIP hashes, and whole TIFF member hashes, not source imagery.

The combined FABDEM area selects N08E050 and N09E050 from one unchanged V1-2 ZIP.
Both native receipts share the ZIP URL and saved connection ID. Each receipt
retains a different source name, immutable snapshot ID, metadata SHA, source
proof, bounds, complete TIFF name, length, and SHA. GLC_FCS10 and GLC_FCS30D
capture the separate W175N70 member of each original official Zenodo ZIP.

Product terms remain distinct: FABDEM CC BY-NC-SA 4.0 and retained Copernicus
notices; GLC_FCS10 and GLC_FCS30D CC BY 4.0. The capture verifies connection and
source selection only. Actual complete download, CRC, decode, raw-pixel, and GUI
acceptance evidence remains in the independently recorded verification files.
