# 数据目录 / Data catalog

GeoD Global 0.2.0 · 2026-10-19

133 项目录 / entries · 17 类 / categories · 110 Ready · 2 Auth · 21 Planned

目录包含具体产品、服务连接方式和本地格式。Ready 表示该入口已开放，源头仍可能需要账号；Auth 表示入口需连接有效授权；Planned 不计为完整接入。部分 Planned 入口提供有限产品操作，其产品范围单独列出。

Entries include products, service connectors and local formats. Ready denotes an available entry, not universal account-free access. Auth requires authorization; Planned is not a completed integration. Some planned families expose a narrowly scoped partial action.

[返回首页 / Home](../README.zh-CN.md) · [Provider integration records](provider-integration-status.md) · [Machine-readable release scope](releases/source-scope.json)

## 光学与航空影像 / Optical and aerial imagery

15 Ready · 1 Auth · 5 Planned

| 产品或连接方式 / Entry | 状态 / Status | 已支持范围 / Scope |
| :--- | :--- | :--- |
| Earth Search | Ready | Sentinel-2 L2A COG |
| Landsat · Planetary Computer | Ready | Landsat Collection 2 L2 |
| NAIP · Planetary Computer | Ready | USDA NAIP RGB + NIR |
| MODIS · Planetary Computer | Ready | MODIS 8-day Surface Reflectance v6.1 |
| Planetary Computer | Ready | Sentinel-2 L2A COG |
| NASA VIIRS · NOAA-21 | Ready | VIIRS VJ209A1 v002 · Provider account: nasa-earthdata |
| NASA VIIRS · NOAA-20 | Ready | VIIRS VJ109A1 v002 · Provider account: nasa-earthdata |
| NASA VIIRS · Suomi-NPP | Ready | VIIRS VNP09A1 v002 · Provider account: nasa-earthdata |
| Copernicus Data Space | Auth | Sentinel-2 L2A SAFE catalog and authorized product access · Provider account: copernicus |
| NASA Earthdata · HLS | Ready | NASA HLS Landsat L30 v2.0 · Provider account: nasa-earthdata |
| Landsat · USGS direct access | Planned | Official catalog and original-product downloads |
| MODIS · Additional products | Planned | One reviewed MCD64A1 h27v05 Item with two complete numerical COG layers; original family remains Planned · September 2020 observations; later processing and Blob dates are separate · Limited partial action; parent family remains Planned |
| VIIRS · Additional products | Planned | Additional science and quality products |
| NASA Earthdata · Additional products | Planned | Additional Earthdata product adapters |
| Jilin-1 / commercial imagery | Planned | Provider catalogs and user-authorized 2D data |
| CBERS-4 / 4A · Surface reflectance | Ready | INPE MUX / WFI L4-SR scenes · individual original bands |
| AMAZÔNIA-1 WFI · L4 multispectral DN | Ready | 64 m L4 original DN · BAND1 download verified with source-bound resume |
| Vantor · Nepal visual original | Ready | Only reviewed acquisition B120001101250A10; publisher cloud cover 93% · Exact 2026-09-07 acquisition, not the event name date or all archive events |
| CBERS-4A WPM · L4 multispectral DN | Ready | Actual official L4 DN scene footprints; no alternative area or level is substituted · 2019 年起；以场景元数据为准 |
| Maxar Open Data · Maui analytic imagery | Ready | One reviewed Maui 2023-08-12 acquisition tile; overlap does not establish valid imagery coverage · Exact original acquisition instant 2023-08-12 21:12:49Z; not a month-long observation |
| EnMAP · Nevada L2A hyperspectral original | Ready | One reviewed Nevada example only; not global scene discovery · Acquired 2023-07-07; processed 2026-05-28 |

## SAR 雷达 / SAR radar imagery

1 Ready · 0 Auth · 1 Planned

| 产品或连接方式 / Entry | 状态 / Status | 已支持范围 / Scope |
| :--- | :--- | :--- |
| Sentinel-1 RTC · Planetary Computer | Ready | Sentinel-1 IW RTC |
| PIESAT-1 · Free SAR samples | Planned | 公告列出的中国及部分海外样例区域 · 公告样例时相；不等于当前所有归档 |

## 地形高程与海底地形 / Elevation and bathymetry

14 Ready · 0 Auth · 1 Planned

| 产品或连接方式 / Entry | 状态 / Status | 已支持范围 / Scope |
| :--- | :--- | :--- |
| Copernicus DEM · GLO-30 Public | Ready | Copernicus DEM GLO-30 Public |
| Copernicus DEM · GLO-90 | Ready | Copernicus DEM GLO-90 |
| NASA Earthdata · SRTM | Ready | NASA SRTMGL1 v003 · Provider account: nasa-earthdata |
| NASA Earthdata · SRTM NUM | Ready | NASA SRTMGL1N v003 source codes · Provider account: nasa-earthdata |
| SRTM GL3 · 3 arc-second elevation | Ready | V3 published 3 arc-second originals · point registration and valid negative heights retained · February 2000 SRTM observations; fixed V3 published archive |
| NASADEM · Public COG archive | Ready | Public elevation COG archive · OpenTopography |
| ALOS AW3D30 · Public COG archive | Ready | OpenTopography 2021 V3.2 DSM archive · not latest JAXA V4.1 |
| SRTM_GL1 · Public COG archive | Ready | OpenTopography public COG tiles · separate from NASA Earthdata HGT delivery |
| NOAA ETOPO 2022 | Ready | 15 arc-second surface elevation and bathymetry tiles |
| USGS 3DEP · Current 1 arc-second elevation | Ready | Approximately 30 m bare-earth DEM · public National Map originals |
| SRTM15+ · Land and ocean elevation | Ready | V2.5.5 published 15 arc-second originals · mixed measured and predicted terrain |
| NOAA ETOPO · Geoid separation | Ready | 2022 EGM2008 vertical offsets · 288 original 15 arc-second tiles |
| FABDEM V1-2 · N08E050 / N09E050 | Ready | Only the two reviewed Somalia-region tiles in the N00E050-N10E060 V1-2 archive · Fixed FABDEM V1-2 derived terrain model; not current observations |
| USGS 3DEP · NJ 1 m DEM | Ready | Only the reviewed NJ_NE6County_B23 x58y451 project tile; not nationwide or seamless 1 m coverage · Publisher source interval 2023-04-13–2025-04-09; 2026-09-17 is publication, not acquisition |
| GEBCO · Regional terrain and global source types | Planned | 全球海洋与陆地；格点间距不代表实测点精度 · GEBCO 2026 grid release; source measurement dates vary · Limited partial action; parent family remains Planned |

## 土地覆盖 / Land-cover products

12 Ready · 0 Auth · 2 Planned

| 产品或连接方式 / Entry | 状态 / Status | 已支持范围 / Scope |
| :--- | :--- | :--- |
| ESA WorldCover · Annual composites | Ready | Annual RGB / NIR, SWIR, NDVI and radar composites |
| Esri / Impact Observatory · Annual land cover | Ready | 10 m annual numerical land-cover classes · Official AWS archive |
| HILDA+ · Historical land use / cover | Ready | 1 km model classes · 1960–2019 · OpenLandMap original files |
| ESA WorldCereal · Mauritius / Réunion | Ready | AEZ 9026 · 2021 · seven annual / seasonal crop themes |
| Copernicus land cover · Permanent-water fraction | Ready | 2015–2019 permanent-water fraction only · continental DE Africa originals |
| JAXA HRLULC · Japan land cover overview | Ready | 2024 Japan classes · 0.025° overview, not the 10 m delivery |
| GLAD · Land cover and use 2020 | Ready | 30 m combined annual classes · 280 published 10-degree originals |
| ESA WorldCover | Ready | 全球陆地；以分幅与掩膜为准 · 2020 / 2021；不当作逐年最新数据 |
| Dynamic World | Planned | Sentinel-2 有效观测覆盖；云与阴影像元有掩膜 · 2015-06-27 起；集合持续更新 |
| GLC_FCS30D · 1985 / 1990 / 1995 | Ready | Eight reviewed W175/W180 tiles from one complete published ZIP · 1985, 1990 and 1995 original class bands only; 2000–2022 annual 23-band files remain unsupported |
| GLC_FCS10 | Ready | Six reviewed legal-coordinate W175 tiles only; publisher W180 edge tiles are not normalized or substituted · 2023 地图；2025-01-24 v1 发布 |
| CLCD · national 2004 land-cover original | Ready | One unchanged author-published national CLCD 2004 original only · 2004 classification reference year; no shared scene acquisition date is fabricated |
| SinoLC-1 · Shanghai classification original | Ready | One fixed historical East_Shanghai original only · March 2023 release; source capture dates vary, with Shanghai timing evidence spanning 2019–2021 |
| RESDC · CNLUCC | Planned | 中国各时相 / 区域；按发布档案确认 · 旧 DOI 与 2026 更新记录分别核查，不混用 |

## 植被与森林 / Vegetation and forest products

10 Ready · 0 Auth · 0 Planned

| 产品或连接方式 / Entry | 状态 / Status | 已支持范围 / Scope |
| :--- | :--- | :--- |
| MODIS NDVI / EVI · Planetary Computer | Ready | MODIS 16-day Vegetation Indices v6.1 |
| DE Africa · Annual fractional cover | Ready | 30 m annual cover percentiles · published Africa tiles |
| Global Mangrove Watch v3 · DE Africa | Ready | 2017–2020 historical mangrove mask · public coastal tiles |
| JAXA MODIS · Monthly vegetation index | Ready | September 2026 monthly maximum NDVI · original DN |
| JAXA PALSAR-2 · Forest / non-forest | Ready | 2020 categorical forest map · JAXA Earth distribution grid |
| JAXA SGLI · Evapotranspiration index | Ready | April 2026 evaporation-efficiency index · valid zero retained |
| GEDI L3 · Mean canopy height | Ready | V2 mean RH100 · OpenTopography reprojected 1 km product |
| ETH · Canopy top height | Ready | 2020 model predictions · published 3° WGS84 tiles · nominal 10 m GSD |
| Impact Observatory · Biodiversity intactness | Ready | 2020 modeled score · one reviewed Beijing-region original |
| Hansen Global Forest Change | Ready | 全球发布网格及数据掩膜 · GFC-2025-v1.13；指标年限分别核对 |

## 水体与水文 / Water and hydrology products

4 Ready · 0 Auth · 1 Planned

| 产品或连接方式 / Entry | 状态 / Status | 已支持范围 / Scope |
| :--- | :--- | :--- |
| DE Africa · Annual water observations | Ready | Annual water frequency and observation counts · published Africa tiles |
| HydroSHEDS v2 · Upstream accumulation | Ready | Americas v2r0 · upstream cells and area · eligible complete originals |
| JRC Global Surface Water | Ready | 全球有效 Landsat 观测区域 · 当前 1984–2024 发布，页面更新 2026-08-26 |
| HydroSHEDS v2 · Flow direction | Ready | 100 officially published Americas tiles; unlisted regions are not supported · HydroSHEDS v2r0 fixed release |
| MERIT Hydro IHU · 5 arc-minute water networks | Planned | Three complete global 5 arc-minute IHU v1.0 originals, 60°S–85°N · Static hydrography model; November 2020 release is not an acquisition date · Limited partial action; parent family remains Planned |

## 海洋 / Ocean products

5 Ready · 0 Auth · 0 Planned

| 产品或连接方式 / Entry | 状态 / Status | 已支持范围 / Scope |
| :--- | :--- | :--- |
| JAXA SGLI · Monthly ocean chlorophyll | Ready | September 2026 chlorophyll-a · original DN and NoData |
| JAXA SGLI · Sea-surface temperature | Ready | September 2026 daytime SST · 0.025° original DN overview |
| JAXA AMSR2 · Sea-surface wind speed | Ready | September 2026 monthly wind DN · source error codes retained |
| JAXA AMSR2 · Microwave sea-surface temperature | Ready | 2026-10-11 daytime original DN · fixed reviewed day |
| NOAA · Ocean heat-content anomaly | Ready | 2020 Q4 0–700 m · WOA09 anomaly · one published global COG |

## 人口与聚居地 / Population and settlement products

7 Ready · 0 Auth · 0 Planned

| 产品或连接方式 / Entry | 状态 / Status | 已支持范围 / Scope |
| :--- | :--- | :--- |
| World Settlement Footprint · DE Africa | Ready | 2015 / 2019 historical settlement mask · public regional tiles |
| GHSL · Built-up surface | Ready | 2020 global 30 arc-second model · square metres per cell |
| GHSL · Built-up volume | Ready | 2020 global 30 arc-second model · cubic metres per cell |
| GHSL · Residential population | Ready | 2020 global 30 arc-second estimates · fractional residents per cell |
| HREA · Settlement electricity access | Ready | Hong Kong 2019 v1 · predicted electrification probability |
| WorldPop · Population grids | Ready | 全球系列与国家专项产品分开 · 2000–2020 historical Global1 unconstrained population counts; no Global2 forecasts in this connector |
| GHSL · Settlement classes 2020 | Ready | One complete published WGS84 original; query/display bounds are its intersection with physical longitude/latitude · Only the modeled reference epoch 2020; not a single observation, all GHSL epochs or future projections |

## 土壤 / Soil products

2 Ready · 0 Auth · 0 Planned

| 产品或连接方式 / Entry | 状态 / Status | 已支持范围 / Scope |
| :--- | :--- | :--- |
| JAXA AMSR2 · Monthly soil moisture | Ready | December 2025 nighttime · original moisture DN and error codes |
| SoilGrids · corrected WRB 2017 | Ready | Only reviewed original tiles 530 and 531 for each of two layers · Corrected SG2017 WRB inputs; no date or depth selection |

## 气候与降水 / Climate and precipitation products

11 Ready · 0 Auth · 1 Planned

| 产品或连接方式 / Entry | 状态 / Status | 已支持范围 / Scope |
| :--- | :--- | :--- |
| CHELSA · Mean climate moisture index | Ready | Fixed 1981–2010 normal · signed CMI original, not current weather |
| JAXA SGLI · Monthly land-surface temperature | Ready | December 2025 daytime · original surface-temperature DN |
| JAXA GSMaP · Precipitation rate | Ready | September 2026 publisher release · original mm/hr and declared time interval |
| JAXA MODIS · Shortwave radiation | Ready | September 2026 release · actual September 1–15 interval and original DN |
| JAXA SGLI · Evapotranspiration | Ready | April 2026 monthly release · original DN and publisher unit conflict retained |
| JAXA SGLI · Aerosol optical depth | Ready | September 2026 500 nm column optical depth · original DN overview |
| JAXA MODIS · Aerosol optical depth | Ready | September 2026 Terra/Aqua T500 · independent calibration |
| JAXA GSMaP · Standardized precipitation index | Ready | September 2026 drought/wetness index · not precipitation rate |
| NOAA NClimGrid · Monthly climate | Ready | September 2022 CONUS · air temperature and precipitation originals |
| ERA5-Land | Planned | 全球陆地区域 · 1950 年至近当前；最近数据更新有延迟 |
| CHIRPS v3 · Monthly precipitation | Ready | 陆地 60°N–60°S；不称全球极区全覆盖 · CHIRPS v3 monthly final product; file availability is checked before downloading |
| WorldClim 2.1 · Nepal climate normals | Ready | Reviewed Nepal NPL original files only · 1970–2000 monthly climate normals, not annual observations |

## 夜间灯光 / Nighttime light products

1 Ready · 0 Auth · 1 Planned

| 产品或连接方式 / Entry | 状态 / Status | 已支持范围 / Scope |
| :--- | :--- | :--- |
| OpenLandMap · Annual nighttime-light DN | Ready | 2024 processed Int16 DN · calibration declarations remain unresolved |
| VIIRS · EOG Nighttime Lights | Planned | 产品发布的全球网格；极区 / 观测条件按产品定义 · 2012 年起，月度 / 年度各版分别核对 |

## 矢量数据与服务 / Vector datasets and services

8 Ready · 0 Auth · 1 Planned

| 产品或连接方式 / Entry | 状态 / Status | 已支持范围 / Scope |
| :--- | :--- | :--- |
| OpenStreetMap / Overpass | Ready | Extract bounded features from a user-provided endpoint |
| OGC API Features | Ready | Discover collections and save complete bounded features |
| WFS 2.0 | Ready | Discover feature types and read GML / GeoJSON |
| ArcGIS Feature Service | Ready | Read public feature layers into managed GeoJSON |
| IRSAMap · Labeled land-cover vectors | Planned | 六大洲 79 个区域，非全球完整地物库 · 2025 年发布；GeoJSON 更新日期 2025-09-21 |
| Microsoft · reviewed building partitions | Ready | All partitions in the pinned publisher index can be searched; individual complete downloads are budget-limited · 2026-08-13 distribution version; imagery dates and model quality vary |
| Overture · original bathymetry part | Ready | Reviewed 2026-09-23.1 base/bathymetry part-00000 only · Pinned 2026-09-23.1 distribution; not every theme or part |
| geoBoundaries | Ready | 各国家提供的行政层级；不保证各国均有同样层数 · 边界源年份与发布版本分别保留 |
| Geofabrik · Monaco free GIS bundle | Ready | Only the pinned Monaco 2026-10-11 free GIS bundle · Dated original extract 2026-10-11; no daily refresh guarantee |

## 地图影像服务 / Map imagery and history

6 Ready · 0 Auth · 1 Planned

| 产品或连接方式 / Entry | 状态 / Status | 已支持范围 / Scope |
| :--- | :--- | :--- |
| NASA GIBS | Ready | Date-aware rendered map imagery via WMS / WMTS |
| WMS | Ready | Save rendered imagery with coordinates and provenance |
| WMTS | Ready | KVP / REST tiles and native-grid image assembly |
| XYZ | Ready | Public PNG / JPEG tile templates in Web Mercator |
| TMS | Ready | Tile templates with rows counted from the bottom |
| ArcGIS MapServer / ImageServer | Ready | Save the public service rendered map for an area |
| Wayback | Planned | 在线影像档案与点位获取元数据 · 版本引用不含影像 · Limited partial action; parent family remains Planned |

## 离线地图 / Offline map archives

1 Ready · 0 Auth · 0 Planned

| 产品或连接方式 / Entry | 状态 / Status | 已支持范围 / Scope |
| :--- | :--- | :--- |
| Protomaps / PMTiles | Ready | Extract a bounded vector tile pyramid from a public archive |

## 栅格目录与覆盖服务 / Raster catalogs and coverages

6 Ready · 0 Auth · 3 Planned

| 产品或连接方式 / Entry | 状态 / Status | 已支持范围 / Scope |
| :--- | :--- | :--- |
| Google Earth Engine exports | Ready | Download GeoTIFF links from your own GEE project |
| STAC API | Ready | Connect a public raster catalog and search its collections |
| Static STAC catalog | Ready | Browse catalog directories and scan items by area and date |
| STAC item | Ready | Read a single item and its declared original assets |
| COG / GeoTIFF URL | Ready | Connect an explicit public raster file URL |
| WCS 2.0.1 | Ready | Get a coverage subset on the service native grid |
| Earth Search · Additional collections | Planned | 有限集合：Sentinel-2 C1 L2A · 额外集合适配器 · Limited partial action; parent family remains Planned |
| Planetary Computer · Additional collections | Planned | One complete reviewed global OISST SST COG; original family remains Planned · 2020-09-01 00:00:00Z–23:59:59Z only; September selector does not enable other days · Limited partial action; parent family remains Planned |
| Copernicus · Additional products | Planned | Additional collections, authorization and product formats |

## 本地文件 / Local 2D files

7 Ready · 0 Auth · 0 Planned

| 产品或连接方式 / Entry | 状态 / Status | 已支持范围 / Scope |
| :--- | :--- | :--- |
| Local PMTiles | Ready | Open a local vector tile archive and retain an offline copy |
| Local MBTiles | Ready | Open local vector or PNG / JPEG tile archives |
| GeoJSON / Overpass JSON | Ready | Inspect local attributes and geometry in a 2D map |
| GeoPackage | Ready | Read local feature layers and preserve the original database |
| Shapefile / ZIP | Ready | Read the original geometry, DBF attributes and encoding |
| OSM XML / PBF | Ready | Read bounded local OSM snapshots and original tags |
| Local GeoTIFF / COG files | Ready | Standalone local raster import and registration |

## 数据门户 / Public data portals

0 Ready · 1 Auth · 4 Planned

| 产品或连接方式 / Entry | 状态 / Status | 已支持范围 / Scope |
| :--- | :--- | :--- |
| ESA · Third Party Missions | Planned | 各任务和集合的可授权归档；按获批项目确认 · 任务 / 归档 / 订单相关 |
| JAXA Earth API | Planned | 各集合覆盖范围；包含 JAXA 与其他机构数据 · 按集合，包括降水、温度、陆面等不同时序 |
| National Earth System Science Data Center | Planned | 中国、典型区域及全球专题产品混合 · 按 DOI 与具体记录 |
| National Tibetan Plateau Data Center | Planned | 青藏高原、中国及全球专题；按具体数据集核对 · 按具体 DOI 与发布版本 |
| Geospatial Data Cloud | Auth | 按各卫星 / 高程 / 专题数据发布范围 · 按实际目录记录 |

