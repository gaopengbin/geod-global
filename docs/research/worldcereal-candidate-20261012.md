# WorldCereal regional-distribution candidate — 2026-10-12

This is new channel research, not an integrated software preset or a native end-to-end acceptance result.

[ESA WorldCereal](https://esa-worldcereal.org/en/products/global-maps) currently publishes 2021 annual and seasonal products, with agro-ecological-zone-specific seasons. [Digital Earth Africa's documentation](https://docs.digitalearthafrica.org/en/latest/data_specs/ESA_World_Cereal_specs.html) describes its African-region copy under CC BY 4.0. The actual publicly listed bucket prefix is `s3://deafrica-input-datasets/esa_worldcereal/`, in `af-south-1`; the product names in the documentation are collection names, not the immediate bucket folders.

The seven documented thematic collections cover temporary crops, active cropland, maize main season, maize active season, maize irrigation, winter cereals and winter cereal irrigation. Seasonal active cropland does not mean maize classification. Adjacent AEZ seasons are not interchangeable calendar-year observations.

Public STAC discovery is currently inconsistent. `https://explorer.digitalearth.africa/stac/` and a real AOI search returned JSON with HTTP 200, but `/stac/collections` and individual WorldCereal collection declarations repeatedly returned an HTML **Internal error** page with HTTP 200. A successful HTTP status is therefore insufficient for a functioning native API connection. The bucket provides standalone publisher `.stac-item.json` documents, but their original asset URLs use `s3://`; the existing generic public connector only accepts stable public HTTPS assets. Preserving original metadata while resolving a reviewed regional HTTPS address would require a scoped adapter, or a clearly labelled direct-file recipe.

One complete original was actually downloaded from:

`https://deafrica-input-datasets.s3.af-south-1.amazonaws.com/esa_worldcereal/activecropland/tc-maize-main/9029/2021/9029_tc-maize-main_activecropland_2020-11-08_2021-05-07_classification.tif`

The independent transfer returned all **3,714,538 bytes** in **7.282 seconds**, SHA-256 `92357b587f8ae61799fa72eebeef030eda78368ba79cf06164331a20022f6d84`. Rasterio decoded all **2,143,621,065** original pixels in **16.812 seconds**: 48,101 × 44,565, one UInt8 band, EPSG:4326, PixelIsArea, approximate 1/12,000-degree grid, embedded NoData 255. The TIFF itself declares CC BY 4.0, product V1.0.0, AEZ 9029 and the actual season 2020-11-08 to 2021-05-07.

Its embedded legend is `0 not active`, `100 active`, `254 No crop`; **254 is a valid non-crop category, not NoData**. All 1,860,684,206 unmasked pixels in this representative happen to be 254, so this sample proves readable numerical classifications but does not verify an active-crop detection. Do not treat it as a positive agricultural demonstration.

Many other AEZ originals exceed 512 MiB or have axes over 200,000–450,000 pixels. The new native 131,072-pixel tiled-image / eight-billion-pixel limits do not make these automatically supported. The publisher's confidence asset can also have a different grid from the classification asset; the tested Ethiopia search returned classification at roughly 1/12,000 degree and confidence at 0.0004 degree. Preserve asset-specific grids rather than inheriting one item-wide resolution for every file.

Raw metadata, HTTP receipts and the fresh complete original remain under the ignored `.verification/worldcereal-candidate-20261012/` directory. No credentials, Earth Engine computation or native product integration were used for this research. A future acceptance run still needs a functioning metadata route, an explicit original selection, complete native download, preview, pixel values and reopened project evidence.
