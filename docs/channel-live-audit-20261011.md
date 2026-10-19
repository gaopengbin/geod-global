# 已接入渠道真实测试 · 2026-10-11

本次使用当前 Rust 下载内核和独立存储，重新请求真实上游。未复用旧原件作为新下载证据。系统代理模式；未核实当前出口国家。

**范围限制**：每个渠道选定样本，不代表所有区域、日期和波段。原始影像单文件最多观察 240 秒，未完成则主动取消并记为未通过，不等同于服务端超时。目录查询、完整文件下载、文件解析、地图渲染和桌面 UI 是不同验收层次。未完成北京市多景全流程和全部断点恢复验证。

| 渠道 | 本次结果 | 已接收字节 | 秒 |
|---|---|---:|---:|
| wms | passed | — | 7.81 |
| wmts | passed | — | 135.86 |
| wmts-rest | passed | — | 76.3 |
| arcgis | passed | — | 12.17 |
| xyz | passed | — | 36.6 |
| arcgis-map | passed | — | 38.2 |
| ogc-features | passed | — | 26.9 |
| wcs | passed | — | 13.7 |
| wfs | passed | — | 5.9 |
| overpass | passed | — | 34.9 |
| planetary-vegetation | timeout_240s | 8863744 | 241.062 |
| planetary-radar | timeout_240s | 8192000 | 240.919 |
| nasa-viirs-noaa21 | requires_account_in_isolated_test | — | — |
| nasa-viirs-noaa20 | requires_account_in_isolated_test | — | — |
| nasa-viirs-npp | requires_account_in_isolated_test | — | — |
| planetary-modis | timeout_240s | 6406144 | 241.138 |
| nasa-srtm | requires_account_in_isolated_test | — | — |
| copernicus-dem | succeeded | 20060161 | 6.029 |
| copernicus-dem-90 | succeeded | 2432905 | 4.017 |
| planetary-computer | timeout_240s | 8110080 | 241.178 |
| copernicus | requires_account_in_isolated_test | — | — |
| planetary-landsat | timeout_240s | 2587705 | 241.102 |
| nasa-earthdata | requires_account_in_isolated_test | — | — |
| planetary-naip | timeout_240s | 9043968 | 241.076 |
| earth-search | succeeded | 242742218 | 18.059 |
| api-sentinel-scl | passed | — | — |
| standalone-sentinel-item | passed | — | — |
| direct-copernicus-dem | passed | — | — |
| PMTiles | passed | — | — |
| TMS | passed | — | — |
| Static STAC | metadata_passed_only | — | — |
| GEE export | expired_link | — | — |

说明：requires_account_in_isolated_test 的原件下载在当前发布策略中尚未开放；未复制用户凭据。Static STAC 本次仅验证目录、项目与重启恢复。GEE 临时链接已失效，不能沿用此前成功结果算本次通过。

初次 XYZ/ArcGIS Map 测试因测试存储非空而中止，改用独立存储后重新通过；TMS 初次范围太小触发颜色丰富度断言，扩大到此前验收区域后完整验证通过。两者均不伪记为源站下载失败。

详细结果：`prototype/qa/channel-live-audit-20261011.json`，原始日志与文件保存在 `.verification/` 对应目录。

Earth Search 初测脚本选择了当前内核不接受的 red 资产；按产品可用的 visual 资产重测通过。NAIP 初测日期无影像，扩至 2020–2025 年检索后实际下载。重测保留独立证据。部分阶段两份原件测试并行，速度是当前环境下的观测值，不是独占带宽基准。

本地 GeoJSON、GeoPackage、Shapefile、OSM、PMTiles、MBTiles 属于本地文件读取，不是上游下载渠道，不计入本次网络验收。未接入、计划中门户也未记为下载渠道。
