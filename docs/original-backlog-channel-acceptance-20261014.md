# 原 Planned 十项接入验收 — 2026-10-14

本轮完成并核销 **10 个原 Planned 稳定 ID，清单外新增 0 项**。目录由本轮起点 **133 / 91 Ready / 6 Auth / 36 Planned** 更新为 **133 / 101 Ready / 6 Auth / 26 Planned**。两个变量、多个波段、多个 ZIP 成员及多个矢量分区均不重复计数。上一扩展批次的“新增十项 / 原核销零项”与历史 85 条目录图片审计保留原记录，不用本轮结果覆盖。[当前待办及基线名单](pending-channels-20261014.md)、[机器可读完整哈希摘要](original-backlog-channel-acceptance-20261014.json)。

十项的真实 Explore / 本地导入、完整文件、原始数值或属性、地图、成果打开和普通重开已经通过；可导出的矢量和本地原件也实际导出并核对完整哈希。Ready 仅对应下面的有限产品与支持范围。其余年份、地区、原件、科学质量或授权没有随状态变化扩大。

## 十个原 ID、科学含义及许可

| 原稳定 ID | 已接入有限范围与原件 | 原值和显示边界 | 许可 / 署名 |
| --- | --- | --- | --- |
| `worldclim` | WorldClim 2.1，Nepal NPL，30 角秒，1970–2000 月常年值；完整 tavg / prec 两个 12 波段 TIFF | 1020×540；Float32 温度 °C、Int16 降水 mm，原 scale 1 / offset 0。波段 1–12 为 1–12 月，不是逐年观测；其他国家/变量/分辨率及 19 波段 bioclim 未替代进来。 | [WorldClim 非商用条款](https://worldclim.org/about.html)；商业与再分发需事先许可，不改写为 CC。 |
| `fabdem` | FABDEM V1-2 的完整 N00E050-N10E060 ZIP，仅接 N08E050 / N09E050 两个完整 TIFF 成员 | 3600² Float32 高程米、Point 原注册、NoData -9999；零和负值有效，原 ZIP 与成员各自保留长度/SHA。 | [作者 V1-2 发布](https://data.bris.ac.uk/data/dataset/s5hqmjcdj8yo2ibzi9b4ew3sn)，CC BY-NC-SA 4.0 及 Copernicus 说明；不套用另一个 Vietnam CC BY 数据集。 |
| `glc-fcs10` | [作者 2023 发布](https://zenodo.org/records/14729665)中完整 W175-W180 ZIP，仅六幅合法坐标 W175 完整分幅 | 单波段 UInt8 分类，未声明标量 NoData，零保留。选定原件约 55k 轴长；地图为预算内 32×32 粗预览，完整原网格/原像元不变。四幅实际超合法经度的 W180 成员不归一化或替换。 | CC BY 4.0；保留作者、发布版本和原分类代码。 |
| `glc-fcs30d` | [作者发布](https://zenodo.org/records/8239305)完整 ZIP 中八幅 reviewed W175/W180 三波段历史原件 | 18554² UInt8，三个波段为 1985/1990/1995，TIFF RGB 标签不变成科学 RGB。原填充码 0/250保留，地图仅显示 1985 的 18×18 粗预览；2000–2022 的 23 波段年度文件仍不支持。 | CC BY 4.0；保留原 Guide、作者与年份语义。 |
| `soilgrids` | corrected SG2017 WRB 250 m 名义原件，most-probable / Acrisols raw score；530 / 531 两瓦片×两层 | 四个完整 4096² UInt8 EPSG:4326 TIFF，scale 1、offset 0、NoData 255；原 0 有效，most-probable 类别 0=Acrisols。score 不除以 100，不推算百分比或概率。不是当前深度属性或历史 1 km IGH 聚合件。 | [原 WRB 发布 README](https://files.isric.org/soilgrids/latest/data/wrb/00_README)，2017 ODbL 1.0；不套用后来版本 CC BY。 |
| `vantor-open` | 仅 Nepal B120001101250A10，获取瞬时 2026-09-07T05:07:53.249593Z，完整 JPEG YCbCr 视觉 TIFF | Y/Cb/Cr 以原像元网格解码，包含原色度子采样的上采样插值与可能的编解码器舍入；灰度预览显示 Y。原云量 93%，不称 RGB、反射率、科学定标或晴空有效覆盖。 | [Vantor 原桶说明](https://vantor-opendata.s3.amazonaws.com/LICENSE.TXT)，CC BY-NC 4.0；公开下载不授予商用权。 |
| `ms-buildings` | 固定 2026-08-13 [发布方完整索引](https://github.com/microsoft/GlobalMLBuildingFootprints)中的整 country / LOD9 gzip GeoJSONL 分区；实测 Andorra 120222210 / 120222212 | 全索引可检索不等于所有分区已下载；完整原件、展开与转换各 20 MiB，50,000 要素 / 500,000 坐标。面积只选整分区，不剪裁/简化；height/confidence 的原 -1 哨兵保留，模型结果非地籍。 | CDLA Permissive 2.0；保留 Microsoft、国家、quadkey、索引版本和原 URL。 |
| `geofabrik` | 仅 [Monaco 2026-10-11](https://download.geofabrik.de/europe/monaco.html) complete free GIS Shapefile ZIP，20 专题层 | 全包原 SHP/DBF 及属性保留，8672 要素；这是发布方衍生 free GIS bundle。raw OSM PBF 在 relation 成员预算处明确拒绝，未借换格式宣称 PBF 成功或所有原 OSM 标签保留。 | ODbL 1.0，保留 OpenStreetMap contributors / Geofabrik 和原 README。 |
| `overture-maps` | 仅 [Overture 2026-09-23.1](https://docs.overturemaps.org/guides/base/) base/bathymetry 的完整 part-00000 GeoParquet；407277 原始行 / 16 行组 | 原 Parquet、WKB、嵌套属性与每行来源保留。地图为 AOI 相交的完整要素 GeoJSON 预览，未剪裁几何；22907 B / 11 要素预览的 bytes/SHA 不冒称 52018294 B 原件。其他主题、分片和建筑未由此验收。 | ODbL 1.0，保留 Overture 及原行 ETOPO/GLOBathy 来源说明。 |
| `local-raster` | 独立用户完整 GeoTIFF 导入/注册、检查、原值、原件导出和 My Data 重开 | 原件拷贝并核对 SHA；输入限 32 MiB / 10000 轴长，原网格和通道保留。本次实际使用完整 WorldClim prec；不推断任意文件的传感器、远程来源或许可证。 | 沿用用户文件真实条款，本例仍是 WorldClim 非商用许可；不补造统一许可。 |

Overture 的原 `depth` 是米制深度级阈值：叠加多边形表示深于该级的水域覆盖，层级可以嵌套；不解释为独立互斥分区或逐点实测深度。[发布方 bathymetry 定义](https://docs.overturemaps.org/schema/reference/base/bathymetry/)、[base 数据说明](https://docs.overturemaps.org/guides/base/)。

Wayback 只提供在线查看和获取元数据，不提供离线影像授权；`earth-search-more` 只接额外 Sentinel-2 C1 L2A 集合。这两个家族仍 Planned，不计入十项核销。GEBCO、SRTM NUM、MERIT、ERA5-Land、EOG、国内门户等仍在原待办，不用其他产品抵扣。

## 软件实际完整文件及双哈希

下表均为本地完整文件 SHA-256，与独立完整原件校核一致。发布方未提供 SHA 的，不称发布方 SHA。ZIP 的成员 SHA 另列；本地用户拷贝与区域预览不增加原产品/渠道数量。

| 完整文件 | 字节 | 完整 SHA-256 |
| --- | ---: | --- |
| WorldClim NPL tavg | 6,827,324 | `68363b342d56bb60066c372875c0a09fd931885f429a2138b2cad7dffec0eaff` |
| WorldClim NPL prec；本地导入/导出同原件 | 4,524,406 | `f6707d2169193e53d86b084f8122a1c6885fa1a1e76435490f0d4fe146c0b7fb` |
| FABDEM 原 ZIP | 21,627,700 | `be348eecf2fb835792c1d446d5528a34956a8509c37004da9b72101c313cacfe` |
| FABDEM N08 完整 TIFF 成员 | 5,592,127 | `6a8988122a922f0f11a059829c4ecfc5451364eca1b38708d02bf789668b7837` |
| FABDEM N09 完整 TIFF 成员 | 16,035,307 | `c8d2ffb9777dddabefa856636a072af81563eb9fed3bf3cfcfebfe95a9ed9928` |
| GLC_FCS10 原 ZIP | 117,500,101 | `60ad302c2e103fb828e2afed348826226db1417704a0902e5e854eaa55a4fae2` |
| GLC_FCS10 W175N70 完整 TIFF 成员 | 47,037,152 | `3e70e7329591c26a90554ec67d4079181260162ce35b1d6df206fcff38c35503` |
| GLC_FCS30D 原 ZIP | 220,928,112 | `776db9239579e2f954621d8fb028be1f1e2bc7b13f4ae02292220459eab81e7f` |
| GLC_FCS30D W175N70 完整三波段 TIFF 成员 | 13,919,236 | `0f5f30202efdd1cfa5e8add2c5869f67f8ca529d70aee0b58a44beb9940fddd9` |
| Microsoft Andorra 120222210 原 gzip | 218,965 | `6fd8cabe2689532388ed37dfba9f1c5e4e9c326a8d8d01874dde76faae41c604` |
| Microsoft Andorra 120222212 原 gzip | 662,138 | `6087ce77db125c9c9e09743924f43379500e3d8d2669569e9e7672be4442b7da` |
| Geofabrik Monaco 原 SHP ZIP | 746,384 | `33611b0f8fd1fbc6562ce6a2c93b35371d26da9e38f3015a889fb9e527e08ccd` |
| Overture 原 GeoParquet | 52,018,294 | `cfaf02a15f70713a52ed2f4367858d1b3d3be0dadba2a9aa4ee7faf0267ea23d` |
| SoilGrids MostProbable 530 | 2,583,826 | `a559810afbd14ceb1e7b5cdbfca645797ac71c30f36a6a0a0d4487faee58e840` |
| SoilGrids MostProbable 531 | 2,430,344 | `5455e644c75cfe06fa9dcca03e7f0d2216d65d984d4fe8cde437b019d892df50` |
| SoilGrids Acrisols 530 | 4,917,543 | `34a45bc93fceecbdec0e38c0a54d4bec6022c353e8b75b994e517c1af5b227b0` |
| SoilGrids Acrisols 531 | 8,042,970 | `46fe919ae7045e1442c18077796147fe26612969d842b4e86e156b810b09b571` |
| Vantor 完整视觉 TIFF | 29,534,465 | `54833b631a8e999ccb6b2aebd409648f79d2a4ece60041b94978aeec009c6f3a` |

ZIP 流程保持原包与完整成员 TIFF 分开的 immutable source proof / source pin / 双长度与 SHA；安全校核保留 CRC、路径穿越、符号链接、加密、重复名和展开/解码预算限制。原包和原 TIFF 512 MiB 预算均未放宽，也没有注入低分辨率替代源。GLC10/30D 的小 PNG 是同完整原件的受限显示；点击原像元仍读取原网格。

微软、Geofabrik 的整原包与完整转换 GeoJSON 分开核对；Overture 的原 GeoParquet 使用专用原件导出路径和完整 bytes/SHA 校验。客户端仍严格核对原 MIME 与源码证明，不把 gzip、ZIP 或 Parquet 伪装为 TIFF/预览 JSON。HTTP 传输 Content-Type 与 reviewed 原包的格式声明分别保留，不以改头冒称原件。

## 实际 GUI 闭环和存储保护

| 原 ID | 实际界面证据 |
| --- | --- |
| WorldClim | 两变量地图、原月度通道、保存与完整下载、原件和普通工程重开。 |
| FABDEM | 最终构建真实合并范围检索两个成员，多选保存新工程 `7208fd83-2ebf-4acb-94f8-0dadcf8ee7ad`，两个完整下载成功；普通重开两幅 Native 缩略图、原像元。 |
| GLC_FCS10 | 真实 Explore 检索/选择、正常完整 117.5 MB 原包、Native 32×32 显示与原网格像元 raw 71。 |
| GLC_FCS30D | 同最终服务版本复用独立 QA store 中已经完成的 220.9 MB 原包；真实 Explore 检索/选择、普通重开 18×18 显示与三年原值 130/130/130，年份/代码说明可见。 |
| SoilGrids | 两个 most-probable 分幅地图与普通重开，530 的原列1820/行0=0，NoData false，Acrisols 图例正确；score 层完整原件/原值校核单独记录，不扩大 GUI 范围。 |
| Vantor | GUI 真实确认新工程 `0df6628c-3eeb-4aa0-a047-d6a6523b5fd0`，原件任务 `7720dd2f-e1c1-4cdc-b552-19ea53b085b3` 完成完整 29.5 MB 原件；普通重开 680×768 Y 预览，原列10000/行10000=Y116、Cb126、Cr125，视觉组件说明保留。 |
| Microsoft | 两分区多选地图，1932 / 5437 要素普通重开；实际导出完整 218965 B 和 662138 B gzip，两份原 SHA 一致；CDLA 标签及原属性保留，导出文件名无重复后缀。 |
| Geofabrik | 8672 要素地图及普通重开；实际导出完整 746384 B 原 ZIP并核对 SHA。 |
| Overture | 11 要素 AOI 地图、原始行嵌套属性，Library/Explore 普通重开；修复拥挤按钮后真实导出完整 52 MB Parquet，SHA 一致。旧误触原属性按钮不算导出通过。 |
| Local raster | 独立导入 4524406 B 原件、12 原通道 `[19,21,49,84,193,345,469,407,275,75,8,11]`，同 SHA 导出，My Data `local-rasters` 标签持久化与原件地图重开。 |

最后纯读核对主 QA store **27 个既有/本轮 jobs 全部 succeeded 且 settled**，并复读本轮 GUI 下载的 WorldClim、两 FABDEM、两 SoilGrids、Vantor 的来源证明、完整原件/成员 SHA 和独立参考像元。27 不是本轮新增渠道或原件数；之前的 jobs、projects、snapshots 和原文件仍原样保存。

GLC30D GUI 验收采用明确归属的两个 QA 服务临时切换；随后立即恢复原端口与原 store，没有复制或迁移数据、没有新下载/POST。恢复回执为 `bothOriginalServicesRestored:true`、`allDataUnchanged:true`、`dataCopyCount:0`、`downloadOrPostCount:0`。独立已经完成的原包没有注入用户主 store。该有限验收不声称每个 archive member 均已 GUI 检查。

原生最终回归 **845 passed / 31 ignored**，桌面 bridge check 与生产前端构建通过；实际完整原件回归另按上述选定源验收。此轮状态/文档变更的 focused 选源与产品回归随源码同步。最终生产前端与桌面调试构建通过（未制作安装包或发行版）；仓库隔离、契约、配方与边界检查通过。桌面权限精确覆盖 148 个命令，仅限本地主窗口。最终运行界面实际读回：已接入 107（101 Ready + 6 Auth）、全部 133、待接入 26、核验中 0。本轮没有提交、推送或部署。
