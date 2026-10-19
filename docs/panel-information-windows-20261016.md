# Panel information windows — 2026-10-16

Explanatory source information now opens outside the work panel through the shared `InfoDetails` adapter. Long product/license/source declarations use a scrollable dialog. Short map error details use an anchored, dismissible popover. Both use existing Beautiful UI controls and Radix interaction primitives, with accessible icon labels, keyboard activation, Escape dismissal and focus return.

Product information sits alongside search, source notes alongside result headings, and file metadata alongside asset selection. This removes full-width explanation accordions from the public-product sidebar. Nested product limitations and request limits are plain sections inside the information window. Source/account eligibility, scientific declarations, metadata, file selection and explicit save/download actions are retained.

Map failures retain their short visible summary and retry action. The technical error appears only in its information popover. At a 300 px test width the actual `MapError` component occupied about 69 px before and after opening its details. This preview used an explicitly labelled offline UI state and did not make a network retry.

The same information-window adapter is used for relevant STAC, WCS, vector, map-service, local-raster, GSCloud, Earth Engine and delivery/processing provenance notes. Interactive task history, editing controls and actual pixel/result output keep their existing behavior.

Verification: 205 affected UI tests across 18 files and seven locale/interpolation checks passed. Reports and screenshots are in `.verification/info-details-20261016/`. A real native CBERS product definition was checked through the isolated QA bridge: the product sidebar had no inline disclosure, and its scroll height stayed 504 px before and after opening the product dialog. Closing returned focus to the same information button. This is browser layout/interaction evidence, not native-window visual acceptance or a new download acceptance.

Temporary QA HTML is removed before building the shipped frontend. No production account or model configuration is changed by these layout checks.
