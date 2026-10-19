# 原待接 ID：Wayback 与分类原件接入核对

本轮检查时间为 2026-10-14；只在独立 Global 仓库工作。保留原稳定 ID，不用 OpenLandMap 夜光 DN、其它土地覆盖、GEE 导出、镜像重新加工的影像或清单外产品替代原待接项。私有原始响应、完整原件和复验脚本在 `.verification/original-backlog-20261014/maps-research/`，不提交原件或私有存储。

## Wayback：真实在线版本与采集来源

`wayback` 新模块沿用已有 Explore 区域、左侧列表、右侧地图和共享组件。读取 [Esri 官方应用的版本目录](https://s3-us-west-2.amazonaws.com/config.maptiles.arcgis.com/waybackconfig.json)，当前真实目录有 197 个版本，2014-02-20 至 2026-09-24。保存原始目录字符串和每个发布方记录，不把 `itemTitle` 的日期当成采集日；不声称这些版本在每个区域都有本地更新。

可筛选发布年份/日期、同时选择最多四个版本作在线渲染预览，在当前缩放级别 10–23 点击地图时查询对应版本的官方 metadata 子图层。全部相交来源记录和 `SRC_DATE2`、`NICE_DESC`、`SRC_DESC`、`SAMP_RES`、`SRC_ACC` 原字段保留；没有元数据时不从发布日期补造采集日。

真实样点 `[116.4,39.9]`、14 级、版本 `63116`：底图发布日为 2026-09-24，元数据原 `SRC_DATE2=1769385600000`，转换日期为 **2026-01-26**，来源 `Vantor / LG06`，原声明分辨率 `0.3 m`、精度 `8.47 m`。这仅是该样点和级别的发布方声明，不能推广为整个 AOI 的统一采集日期或实测精度。[Esri 官方说明](https://www.esri.com/arcgis-blog/products/arcgis-living-atlas/mapping/use-world-imagery-wayback)明确版本发布时间与影像实际采集日期的区别；当前 metadata 查询路径和字段由 [Esri 官方库](https://github.com/Esri/wayback-core)复核。

此入口提供在线瓦片显示、原始来源信息查询和不含影像的版本/AOI参考 JSON。当前 [官方图层条目](https://www.arcgis.com/home/item.html?id=ab83ada1bd3145978eecfb115aaa10c3)的原 `licenseInfo` 明确该图层不用于离线瓦片导出；[官方 Wayback Export 说明](https://www.esri.com/arcgis-blog/products/arcgis-living-atlas/imagery/wayback-export)要求 ArcGIS 组织账号，导出的瓦片包仅供在 ArcGIS 内使用。界面提供带真实区域和版本 hash 参数的官方应用链接，**没有调用通用 native map-image 保存/ZIP 导出，也没有宣称授权离线导出已经完成**。在线瓦片颜色不是原始科学波段。

新文件 `wayback-client.js`、`wayback-explore-ui.jsx` 复用 `ExploreSourceFrame`、`ServiceExploreMap`；后者仅新增 `wayback` 在线 TileLayer 分支。六项纯契约测试及三组相关界面回归共 34 项通过，覆盖目录/模板身份、发布日期与采集字段分离、区域切换取消、晚到响应、最多四层、来源归属及元数据错误。S3 与 metadata 端点实际返回 HTTP 200、JSON MIME 和 `Access-Control-Allow-Origin: *`。这些测试/HTTP 记录不代替主任务的实际浏览器视觉验收。原稳定 ID 的离线原件导出门槛仍未通过，不按完整下载 Ready 核销。

## GLC_FCS10：完整原作者包和六个原成员

原 ID `glc-fcs10`，使用 [原作者 Zenodo v1](https://zenodo.org/records/14729665)；记录许可为 CC BY 4.0。2023 是地图参考年，2025-01-24 是发布日；产品是分类地图，不是原始 Sentinel 波段。[原论文](https://essd.copernicus.org/articles/17/4039/2025/)和随记录发布的指南、分类 legend 作为解释依据。

完整官方包 `GLC_FCS10maps_2023_W175-W180.zip`：

| 核对项 | 实际独立回执 |
| --- | --- |
| 完整字节 | 117,500,101 |
| 发布方 MD5 | `b940ca1c7269fe024827248cf1eb25e5`，与完整本地文件一致 |
| 完整 SHA-256 | `60ad302c2e103fb828e2afed348826226db1417704a0902e5e854eaa55a4fae2` |
| ZIP 成员 | 10 个 TIFF，全部 ZIP CRC 核对通过 |
| 可接范围 | `W175N55/N60/N65/N70/S10/S15` 六个完整 TIFF |
| 原始编码 | UInt8、单波段、LZW、256×256 tiled、EPSG:4326、PixelIsArea |
| 原始比例/缺失值 | scale 1、offset 0、TIFF 未声明 NoData；零值保留，不能擅自设为 NoData |
| 原始格网 | 约 55,660–55,673 像素一边，间距 `0.00008983152841195215°`，无内部 overview |

每个可接 TIFF 独立 SHA、字节、真实变换/范围和八个实际原像元在 `glc-fcs10-reviewed-index.json`；参考解码已包含北半球与南半球、零、210 和 51/52/61/62/71/81/121/122/130 等存储码。所记录的灰度预览使用原分类码，不推定物理单位、质量掩膜或某个地区的准确率。

其余四个 `W180` 原成员实际 GeoTIFF 经度从约 `179.999925` 到 `185.000038`，超出现有合法地理 footprint 契约。保留完整 ZIP 中的原字节，配方暂不选择这些成员；没有把它们移经度、重写 transform、重采样或替换成相邻示例。文件名也不当作精确范围，原始的小幅边界重叠保留。

新增 `datasets/glc_fcs10.rs` 以原 ID 注册六个精确原成员、固定 2023 `classes` 图层；通过共享安全 ZIP foundation 将完整 ZIP 与所选完整 TIFF 分别保留并校验。没有给 ZIP 编造 TIFF MIME、发布方 STAC 或下载后的生成 AOI 文件。现有 512 MiB 双预算、原始网格、轴、解码内存、块与预览时间限制保持。**独立完整原件、参考像元、原生函数验证和下述主软件真实新任务验收均已通过；产品 UI 最终验收由主任务另报。**

六件原 TIFF 的完整字节 SHA 和全部 48 组原生像元已与独立参考逐一通过。无 overview 的原件先计算实际请求块集合和保守解码费用，再逐格寻找 512 MiB 累计预算内最大的浏览网格，只有精确 SHA 和完整 profile 白名单允许使用该策略。`W175N70` 与 `W175S10` 的原生 PNG 均为 32×32、1024 个源块、536,870,912 B 费用；33 像素边长会超预算。两图全部 alpha=255，分别有 9/5 个实际灰度，已实际查看。只供低细节浏览；完整原件、分类码、原始网格、GeoRef 和任意全分辨率像元不变。此原生函数测试仍不代替主任务的新软件下载和浏览器闭环。

**主软件真实新任务验收已通过**：主任务创建的 `89d406f6-1232-41a2-82a9-164c32a363a0` 在同一冻结 runtime `2f477ffe4d989853a524c8952d8f0000ec66831450a44ca1ca07c6abab02d405`、原 4318 存储中，从作者网站实际完整下载 117,500,101 B ZIP，最终 attempt4 succeeded/settled，SHA 与上表一致。接手时 attempt3 的网络中断已保存 22,128,652 B；独立重算原生 v2 检查点 SHA 一致后，只经同任务 API retry 完成续传，没有注入/复制研究原件、更换 runtime、修改存储文件或其它任务。所选整个 `GLC_FCS10_2023_W175N70.tif` 为 47,037,152 B、SHA `3e70e7329591c26a90554ec67d4079181260162ce35b1d6df206fcff38c35503`，native archiveOriginal 两层回执一致。

该真实软件任务的八组 UInt8 原分类码（包含零）、原标量 NoData=null 与原生 noData=false、GeoRef 和 32×32 非均匀/全不透明 PNG 已与独立参考复验；inspect、snapshot、job 的完整 API 重读一致。验收时全部 24 个主软件任务均 settled，只有在报告该状态后才交由主任务安全刷新；本分支没有停止或替换主软件。证据在私有 `maps-research/root-glc10-acceptance/`，最终产品界面层验收仍由主任务另报。

## GLC_FCS30D：三个历史分类年完整原件

原 ID `glc-fcs30d` 使用 [原作者 Zenodo 8239305](https://zenodo.org/records/8239305) 的完整 `GLC_FCS30D_19852022maps_W175-W180.zip`，许可 CC BY 4.0。完整 220,928,112 B、发布方 MD5 `c08709165b8f07a19d861cc5a5afe876` 和本地完整 SHA-256 `776db9239579e2f954621d8fb028be1f1e2bc7b13f4ae02292220459eab81e7f` 均已核对；16 个 TIFF 的 ZIP CRC 全部通过。保留全部 ZIP 和所选整个 TIFF 的独立回执。

作者指南明确老文件名含 `19852000` 的半五年 TIFF **只有 1985、1990、1995 三个波段，没有 2000 波段**。八个可接成员为 W175N55/N60/N65/N70、W180N55/N65/N70/N75，18554×18554×3 UInt8、LZW、256×256 tiled、EPSG:4326、无 overview。本产品的 W180 原 GeoRef 实际为 -180 附近，在合法范围内；并未套用 GLC_FCS10 的经度问题或改坐标。TIFF Photo=RGB 是存储标签，这三个通道是分类年，不能显示成光学 RGB 波段。当前 `classes-1985` 固定首波段 1985 灰度；完整文件与原像元数组保留另外两年，不能假称任意年份切换已经完成。[作者论文](https://essd.copernicus.org/articles/16/1353/2024/)及原包用户指南用于解释。

独立解码和 native 对全部八件原件的 64 组原像元数组精确一致。指南中的 0/250 为填充码，但原 TIFF 无标量 NoData；保留声明 null 和原始填充数值，不自行添加掩膜。`W175N70` 与 `W180N70` 原生 PNG 均为 18×18、324 个源块、509,607,936 B（486 MiB）费用，19 像素边長拒绝；全 alpha=255，6/7 个灰度值，已实际查看。预览只供粗浏览，原文件、三年全分辨率值与地理参考不改。年度 2000–2022 另有 23 波段原件，仍超现有 1–16 samples 上限，不丢波段、不假造四波段，也不将其计入可接范围。模块、严格源绑定、完整原件 native 测试和下述独立真实软件任务均通过；产品 UI 最终验收由主任务另报。

**新增真实软件任务验收已通过**：使用当日构建后冻结的 runtime SHA-256 `2f477ffe4d989853a524c8952d8f0000ec66831450a44ca1ca07c6abab02d405`，独立空存储、loopback 45237，真实 `datasets/connect → 新项目 → 官方完整 ZIP 下载 → inspect/pixel → 同冻结软件重开`。没有复制研究原件填充软件下载目录。项目 `a0c2e1ed-9945-4dab-b0c1-49d391c5e2c8`、任务 `d7c018b5-760b-43b6-a931-d2f02fc79e30` 最终 succeeded/settled，整个 220,928,112 B ZIP SHA 与上表一致；完整所选 `W175N70` TIFF 为 13,919,236 B、SHA `0f5f30202efdd1cfa5e8add2c5869f67f8ca529d70aee0b58a44beb9940fddd9`，native archiveOriginal 两层回执一致。八组原三年像元数组、实际 GeoRef 和原生 18×18 PNG 再次通过。软件重开后 jobs、projects、发布方 snapshots 完全相同，完整原件及八组像元再读一致。

源站数次中断后，通过同任务的四次原生 API retry 实际续传；每次重试前独立重算已有部分文件 SHA，对应 native v2 检查点的字节从 18,460,644 单调增加至 146,164,088、146,579,768，再完成全包。最多八次重试、连续两次无进展则停止；非网络错误不重试。完整原包 SHA 仍必须最终匹配，未放宽预算或伪造 source/ETag。全部失败、部分 SHA、重试、最终回执、原像元和重开证据在私有 `maps-research/fresh-native-glc30d/`。这是独立真实软件任务 acceptance，主任务的产品浏览器层验收另报。

## Vantor 当前计划：一个完整视觉原件

原 ID `vantor-open` 接入当前官方桶的 `Nepal-Flooding-Aug-2026` 中 `B120001101250A10`。当日 [原 item](https://vantor-opendata.s3.amazonaws.com/events/Nepal-Flooding-Aug-2026/B120001101250A10.json)全文与已完整获取的原件对应记录一致；[原事件 Collection](https://vantor-opendata.s3.amazonaws.com/events/Nepal-Flooding-Aug-2026/collection.json)原文 CC-BY-NC-4.0 和 item membership 保留并严格绑定。观察时间 **2026-09-07T05:07:53.249593Z**，不能用事件名的 August、事件发生日、发布或 ingestion 日期替代。原 cloud_cover=93，不能承诺有效 AOI、无云或洪灾识别。

完整原件 29,534,465 B，SHA-256 `54833b631a8e999ccb6b2aebd409648f79d2a4ece60041b94978aeec009c6f3a`，36389×41157×3 UInt8、EPSG:4326、Photo6 YCbCr、JPEG7、512×512 tiles、2×2 色度抽样、RefBlackWhite=[0,255,128,255,128,255]，八级原图/缩略图，未声明标量 NoData。只有精确 SHA、全部原 profile 和 codec 标签符合才允许 native 视觉解码；其它 YCbCr 文件仍明确拒绝，不泛化放行。

TIFF 原压缩块通过独立 `tifffile` 与 `imagecodecs` 的 libjpeg_turbo 3.2.0、`outcolorspace=YCbCr/fancyupsampling=True` 核对，八组原生 Rust JPEG 解码像元全部精确一致，包括 [116,126,125]、[133,125,125]。报告的值为 **JPEG 解码并色度上采样到原像元格网的 Y/Cb/Cr**，不是 RGB、原始抽样色度平面、无损传感器样本、辐亮度或反射率；其它解码器可能有舍入差异。原 publisher `eo:bands` 逻辑 RGB 声明保持原文，另有 optional `sampleEncoding` 明确 Y/Cb/Cr、JPEG、2×2、triangular fancy 上采样、Y-only 预览与 scientificCalibration=false。其它科学/分类原件不新增此字段。

原生 PNG 实际为 680×768、alpha 全 255、灰度变化正常，已查看有云/雪山和黑色画布；显示只做 Y 灰度，不能把黑色或 noData=false 解释为有效覆盖。完整原件测试、像元、预览已经通过；源 proof/Collection/完整 SHA 的防篡改回归也通过；软件新任务/浏览器验收由主任务另报。范围只这一件已核验的当前 visual，不宣称所有当前事件或商业订购 Ready。

## Earth Search 扩展：原家族部分完成

`earth-search-more` 新有限模块只增加官方 [sentinel-2-c1-l2a](https://earth-search.aws.element84.com/v1/collections/sentinel-2-c1-l2a) 集合，原生 STAC 查询仍保留 AOI、UTC 月、真实 item 与原 band 文件。当日原集合为 license=proprietary 并链接 Sentinel Legal Notice；保留原声明，不改成 CC 或将匿名访问理解为无限制用途。并未认定全部九个集合完成，**不抵扣原待接十项核销目标**。

实际 item `S2C_T50TMK_20250930T030332_L2A` 的 B01 原 TIFF 3,949,171 B、SHA-256 `3690be38900749e60fd23b63d41c77513ce9fc61794cf48e61a0705e6752b87c`；1830² 单 UInt16、EPSG:32650、60 m、NoData 0。STAC 与 TIFF 嵌入 scale 0.0001、offset -0.1 一致，原值样点 1304/1355/1133 保留。B01 是 coastal/aerosol 光谱波段，不是气溶胶光学厚度。另一个 L1C 集合原 S3 JP2 仍明确不支持；不能拿此 L2A TIFF 替换。独立完整原件核对和模块月/区域/集合严格测试不能冒称全部家族验收。

## 原清单后续候选与实际阻碍

| 原稳定 ID | 官方实际结果 | 接入边界 |
| --- | --- | --- |
| `srtm-num` | NASA CMR `C2763266368-LPCLOUD` 返回真实 `SRTMGL1N.003`、NUM ZIP 和 2000-02-11 至 2000-02-21 时间；该真实小 ZIP 匿名请求返回 401 | 需要用户已有 NASA Earthdata 授权和专用一字节 NUM 解码；不能换 NASADEM NUM、高程 HGT 或 AppEEARS 导出。[CMR 正式集合](https://cmr.earthdata.nasa.gov/search/collections.json?short_name=SRTMGL1N) |
| `merit-hydro` | 作者当前入口要求 Google Form 许可登记、邮件密码及 Dropbox；版本仍为 v1.0.1 | 当前没有下载密码，未读取完整原件。双许可 CC BY-NC 4.0 / ODbL 1.0，不能将其它汇流产品替代。[作者入口](https://global-hydrodynamics.github.io/MERIT_Hydro/) |
| `eog-nightlights` | 官方登记页明确自 2026-06-01 起，OpenID 程序化访问只供付费订阅者；EOG 原件仍是辐亮度产品 | 当前没有订阅凭据，不能用作者加工的 OpenLandMap DN 或 Earth Engine 导出清掉此 ID。[官方访问政策](https://eogdata.mines.edu/products/register/) |
| `glc-fcs30d` | 完整原包、16 个 TIFF CRC、八件 1985/1990/1995 全三波段原件及 native pixels/预算 PNG 已通过；独立真实软件新任务全包下载、两层回执、三年像元和重开也通过 | 有限历史分类范围已落模块；2000–2022 年度 23 波段仍超现有 1–16 sample 上限，不静默丢波段或放宽限制。产品浏览器层验收由主任务另报 |
| `clcd` | 原作者 [省分包记录](https://zenodo.org/records/5816591)中最小数据 ZIP 为 878,227,762 B，全部数据原包超 512 MiB | 分类表不能替代地图；不从超预算原包只截取一个成员冒称完整包下载 |
| `sinolc-1` | 作者完整目录中 Tianjin/Shanghai ZIP 和更新 Zhuzhou TIFF 有小于 512 MiB 的候选；Zhuzhou 的三个真实 206 头部范围读取已证实 108,000×186,000、20,088,000,000 像元 | Zhuzhou 的轴和像元都超过现有预算，未整件下载或注册；附属 `Image_capture_time.tif` 不能替代土地覆盖图清账。[作者系列目录](https://zenodo.org/search?q=parent.id%3A7707461&f=allversions%3Atrue) |
| `dynamic-world` | 当前 [Google 官方目录](https://developers.google.com/earth-engine/datasets/catalog/GOOGLE_DYNAMICWORLD_V1)仍给出 `GOOGLE/DYNAMICWORLD/V1` Earth Engine collection；每个 Sentinel-2 L1C 观测对应九类概率与 label，官方页面展示 Earth Engine 查询/visualize，不提供已核验的匿名完整原 TIFF href | 当前只证实官方 Earth Engine 身份与 CC BY 4.0，未获得无需平台授权/任务配额的发布方完整原件。遵守本轮不使用 GEE 配额或导出替代原件要求；在线可视化颜色、单 label 导出或另一个土地覆盖产品不能清除此 ID |

源站账号、格式或容量阻碍单独保留，不改成“连接已成功”或“已完成原件下载”。所有真实完成条件由主任务在共享注册、实际软件任务及原 Explore 流程通过后逐项确认。
