# 本地矢量与 OSM 提取文件

在线来源现另支持 [WFS 2.0](wfs-features.md)。WFS 原始响应使用受管归档保存，地图和导出读取经验证转换的 WGS84 GeoJSON。本地 [GeoPackage](geopackage-local.md) 另提供完整原数据库、多图层、原属性和明确坐标转换；[Shapefile](shapefile-local.md) 支持 SHP 伴随文件 / ZIP、多图层、原属性与原文件组导出，范围与证据分别记录。

2026-10-03。桌面版“我的数据 → 矢量文件”可以打开 WGS84 GeoJSON、包含完整嵌入几何的 Overpass JSON、独立 GeoPackage 文件和 Shapefile。在文件卡片中打开工作空间后，可以查看实际点、线、多边形、孔洞与属性，调整详情面板宽度，并导出 GeoJSON。本文记录 GeoJSON / OSM 文件工作流与原验收；另已接入[用户配置的 OSM Overpass 在线提取](osm-overpass.md)，不预设公共后端。完整 [PMTiles](pmtiles-local.md) 与 [MBTiles](mbtiles-local.md) 通过独立“离线瓦片”入口使用。

桌面文件选择器默认“引用原文件”：只登记路径与文件信息，不隐式复制或改写原件。需要独立副本时，明确选择“复制到 GeoD 数据目录”。开发浏览器只提供“导入矢量副本”，不会把浏览器文件当作可以持续访问的原生路径。移除登记不删除原文件，也不作为清理托管副本的操作。文件登记与来源哈希会持久保存；每次打开、导出均重新核对完整来源哈希和几何信息。原件移动、丢失或修改后应重新打开文件。

GeoJSON 使用 RFC 7946 的 WGS84 经纬度，不猜测投影；支持 Feature / FeatureCollection 和七种几何类型。原始要素编号、属性、第三维坐标、孔洞及集合外的成员保留在导出内容中。地图使用内部独立编号，避免重复原始编号导致要素丢失；任意属性不能覆盖地图几何或注入 HTML。普通 GeoJSON 不推断许可，文件原有许可声明可随内容保留，但不会因格式而认定已获得使用授权。[GeoJSON 规范](https://datatracker.ietf.org/doc/html/rfc7946)

Overpass JSON 支持 node、完整几何 way，以及由完整 outer / inner 成员构成的 multipolygon / boundary relation；分段和反向线段可以拼接。新登记还支持完整的嵌套关系 GeometryCollection，并保留 node 引用、成员 role 和源修改元数据；旧登记保留原转换器与校验值。道路闭环默认仍为线，明确区域标记和固定分类表中的区域标签才生成面。保留 OSM 类型、编号、全部字符串标签、数据更新时间、OpenStreetMap 署名与 ODbL 链接。输入若报告失败 / 超时、缺成员、连接或内环归属不明确，整份文件报错，不静默丢弃要素或填补边界。[Overpass 几何输出](https://dev.overpass-api.de/overpass-doc/en/full_data/osm_types.html)、[OSM 署名与许可](https://www.openstreetmap.org/copyright)

单文件限 20 MiB、50,000 个要素、500,000 个坐标；登记限 1,024 份。GeoJSON 不推测投影；GeoPackage / Shapefile 使用来源定义和记录的转换方法，详见[坐标转换范围](geopackage-local.md)及 [SHP 限制](shapefile-local.md)。现另支持[本地 OSM XML / PBF](osm-local-data.md)有界完整快照，原件、对象元数据与独立验收分别记录；大文件 / 不完整提取、通用重投影、自动拓扑修复、跨日期变更合并、栅格叠加和工程内矢量处理仍待接入。本地工作空间采用已随应用提供的 Natural Earth 底图，不请求 OSM 公共标准瓦片；在线提取需配置用户自有或合适授权的服务，不能把公共 Overpass 实例作为应用默认批量后端。[Overpass 公共实例使用说明](https://dev.overpass-api.de/overpass-doc/en/preface/commons.html)

对 GeoJSON / Overpass JSON，整数原文超过 `±9007199254740991` 的输入会报错，应在来源 JSON 中将其保存为字符串，避免编号 / 属性被浏览器舍入。GeoPackage 的超大整数使用声明的十进制字符串编码，原数据库仍保存原值；Shapefile 的全部 N/F 数值字段使用原文十进制字符串，原 DBF 不改写。普通浮点值按双精度读取；原验收中的真实 OSM 数据的全部坐标与标签已逐项核对。

## 已完成验收

一份人工进行的小范围真实 Overpass 查询得到 197,463 字节文件，SHA-256 为 `69ebac14ff6089c323f604755ca122d3f93d382eadd1d502a576b50aee122a0d`，数据时间为 `2026-10-02T08:48:21Z`。文件包含 90 点、136 条线、14 个多边形、7 个多多边形，共 247 个要素、1,309 个坐标、12 个孔洞。

全部编号、标签、坐标顺序及精度均核对；关系几何使用独立 GEOS polygonization 参考，全部几何一致且有效。真实原生服务中完成引用、明确复制、导出、GeoJSON 再打开及重启恢复。无头浏览器分别检查 1440 像素英文浅色和 1024 像素中文深色界面：实际地图绘制、点击属性、面板调整、真实文件内容导入 / 移除登记及完整导出内容。未操作用户桌面；原生文件选择器和桌面 WebView 的人工验收仍保留为独立步骤。证据见[验收记录](../prototype/qa/vector-native-verification.json)。

重跑独立几何校验：

```powershell
python -X utf8 scripts/verify-vector-osm.py --source SOURCE.json --export EXPORTED.geojson --registry DATA/vectors.json --report REPORT.json
```

校验脚本仅在 QA 环境使用 Shapely，不增加应用依赖，也不会联系网络或向产品数据目录写测试记录。命令行同样支持 `vectors open --file FILE --data-dir DIR`（默认引用）、显式 `--mode managed`、`vectors list|inspect|forget` 与 `vectors export --id UUID --out FILE --data-dir DIR`；原生路径只能通过直接模式使用，HTTP 适配不接受读写本机任意路径。
