# 下一批公开原件研究：HydroSHEDS 汇流积累与 NOAA 大地水准面 — 2026-10-14

本组接入两个具体产品：HydroSHEDS v2r0 汇流积累（ACC / ACA 两个科学层，共计一个产品）和 NOAA ETOPO 2022 v1 大地水准面分离值。前者不是已接入的 DIR 流向；后者不是已接入的 ETOPO 地表高程。现有通用直接 GeoTIFF 路径读取发布方原件，不创建发布方未提供的 STAC，不执行区域导出或服务端计算。

## 官方目录与有限接入范围

| 产品 | 今日读取的真实发布目录 | 原件头部核查 | 接入范围 |
| --- | --- | --- | --- |
| HydroSHEDS v2r0 ACC / ACA | [官方产品下载页](https://www.hydrosheds.org/downloads-core-data-v2)，其页面引用 [20260830 发布索引](https://data.hydrosheds.org/file/hydrosheds-v2/download-index.json?v=20260830) | 对官方文件体量筛选后的每个真实 URL 发出 32 KiB HTTP Range，须响应 206；从原 TIFF 标签读取 dtype、地理 CRS、实际变换/外边缘与 NoData，并读取 `Content-Range` 精确完整体量 | 40 个 ACC、27 个 ACA 文件，均不超过既有 512 MiB 上限。未纳入更大原件，不从另一个版本或地区替代。原始官方索引有 100 个美洲分幅，并不代表全部都能在当前体量上限下下载。 |
| ETOPO 2022 v1 geoid | [NOAA 15 角秒 GeoTIFF 原件目录](https://www.ngdc.noaa.gov/mgg/global/relief/ETOPO2022/data/15s/15s_geoid_gtif/) | 288 个发布文件各真实 32 KiB Range / 206 核查，实际均为 3600² Float32、EPSG:4326、Point 注册、无 NoData 标签，精确字节数均不超过既有上限 | 288 个固定发布原件；没有启用 surface、bedrock、SID 或服务生成的 AOI 子集。 |

355 / 355 个元数据头部检查成功，累计最多约 11.1 MiB 头部字节，无原数据瓦片计算、账号或 GEE 额度调用。这个范围核查不等于 355 个完整原件及地图全部验收。完整原件独立下载与全像元解码验收目前仅下表三件。

编译期固定索引 `crates/geod-runtime/src/datasets/next-hydro-geoid-reviewed-index.json` 只保存实际发布 URL、实际原件外边缘、精确字节数、数据类型、原值尺度/NoData 与头部 SHA256。完整文件 SHA256 仅给确实已完整下载的三件；其他记录的 `sha256` 为 null，`headerSha256` 不冒充完整文件摘要。接入最多一次选择八件相交原件，区域过大返回错误，超限件不默默忽略后宣称全区域已完成。只有地理查询边缘与合法经纬度相交；原始 TIFF 外边缘、浮点精度和文件字节保持不变。

## 三件完整发布原件与独立检查

| 原件 | 完整体量 / 今日研究下载用时 | 完整 SHA256 | 全像元独立解码 |
| --- | --- | --- | --- |
| [n00w050 ACC](https://data.hydrosheds.org/file/hydrosheds-v2/ACC/1s/n00w050_ACC_1s_v2r0.tif) | 76,824,474 B / 6.531 s | `5a67c5fb2705b65117546dadc9437275a0d2ea9a2ecdd33b7eb59de3d35e14b4` | 36000² Float64；2,688,136 个有效像元、1,293,311,864 个 NoData；有效范围 1–44,106,142。 |
| [n00w050 ACA](https://data.hydrosheds.org/file/hydrosheds-v2/ACA/1s/n00w050_ACA_1s_v2r0.tif) | 79,213,241 B / 85.141 s | `f4dd75675c54c74656bca10bdf010500421b3c9fc286095090f24a299b0edf93` | 36000² Float64；与 ACC 相同有效/NoData 个数；有效范围 0.0009493472753092647–41878.26953125 km²。 |
| [N45E105 geoid](https://www.ngdc.noaa.gov/mgg/global/relief/ETOPO2022/data/15s/15s_geoid_gtif/ETOPO_2022_v1_15s_N45E105_geoid.tif) | 19,941,002 B / 13.257 s | `5664f943ad524f4415cbdb068962d4b26e9f3ae0364f0662ea592e9561fa2ff8` | 3600² Float32；全部 12,960,000 个像元为有限值；范围 −46.710445404052734–7.840210437774658 m。 |

三件完整研究原件共 175,978,717 字节。以上用时是该次真实研究传输的端到端实测，不是恒定下载速度或软件原生下载回执。各原件以分块遍历所有像元，未把 HydroSHEDS 约 10.4 GB 的单幅未压缩数组整体装入内存。原件没有被研究脚本改写、裁剪、重投影或放入软件私有存储。

ACC / ACA 为 LZW 压缩的 512² 数据块，原件内有 2、4、8、16、32、64 倍缩小层。地理足迹接近 50–40°W、0–10°N，但 ACC 原始外边缘有约 1e−14 至 1e−9 度浮点偏差，ACA 标签更整齐；实际标签分别保存，不强制使二者变成同一人为网格。这个分幅绝大多数为海洋 NoData，有效陆地位于巴西 Amapá 海岸。真实流程 QA 用区域为 `[-49.9,0.2,-49.4,1.0]`，范围内列 360 / 行 32400 的 ACC 原值是 4，ACA 是 0.0037985334638506174 km²；这不是替换用户区域的默认演示。

Geoid 原件的真实范围是 `[105,30,120,45]`，15 角秒点注册，无内部缩小层，Deflate 压缩、256² 数据块，TIFF `units=metre`。列 1800 / 行 1800 原值 −20.39291763305664 m、无缺测。负值不是水深，原件没有声明 NoData，因此不能将 0 或任意负数当作缺测。原始版权标签是 `Derived from work by NGA. Public Domain`，原始说明包括 `egm08_25.gtx`、2018 年源文件与 2019 年 TIFF 时间；这些不是地形采集日期。

## 科学解释与许可

[HydroSHEDS v2 技术说明 2.0.0](https://data.hydrosheds.org/file/technical-documentation/HydroSHEDS_TechDoc_v2_0_0.pdf)说明：ACC 包括当前像元，首个上游值为 1，表示汇入像元数；由于经纬度网格的像元面积随纬度变化，不能乘固定 30 米方形面积当成真实上游面积。ACA 才是按纬度修正的平方千米，并包括当前像元本身的面积。二者都是从水文校正地形路由推导的指标，不是河流流量、降雨量、实测洪水或水体积。原始 Float64、尺度 1、偏移 0 和 `1.79e308` 缺测码保持；灰度 stretch 仅用于显示。

HydroSHEDS v2 技术说明采用 CC BY 4.0，同时请求未经作者书面许可不要在其他网站整体镜像不变原件。该额外表述作为发布方请求保留，没有把它改写为许可证禁止商业使用，也没有据此建立自有镜像或对外发布。软件由发布方分发 URL 读取，保留 HydroSHEDS / DLR / WWF / Confluvio 署名和指定论文引用。产品页面、原件成功下载及算法正确性须分别判断。

[NOAA ETOPO 用户说明](https://www.ngdc.noaa.gov/mgg/global/relief/ETOPO2022/docs/1.2%20ETOPO%202022%20User%20Guide.pdf)中的 geoid 是 EGM2008 大地水准面相对于 WGS84 椭球的垂直分离值。NOAA 采用的转换约定为：ETOPO 正高 + GEOID = WGS84 椭球高。此次软件只交付原始 geoid 数值，不替用户做垂直基准运算、潮汐转换或导航。原件水平 CRS 是 EPSG:4326；其标签描述的 EPSG:4979 / EPSG:3855 是垂直运算语义，不能因此给水平地图换 CRS。

[NOAA 官方产品元数据](https://www.ncei.noaa.gov/access/metadata/landing-page/bin/iso?id=gov.noaa.ngdc.mgg.dem%3Aetopo_2022)载明 CC0 1.0，并明确不用于导航。保留原件 NGA 公共领域声明、NOAA 来源、版本和 DOI。固定 EGM2008 垂直基准场不是当年遥感观测、海底新测资料、潮汐或海图基准。

## 软件与 UI 验收状态

主任务已通过真实 Explore UI 在本批独立 store 发起三件原件的保存与完整下载。随后本组只发出 GET 读取这些最终成功 jobs、projects、snapshots、原生检查与原值；未另外下载、未 POST、未将研究文件注入 store。三件软件原件完整 SHA / 175,978,717 字节均与独立研究原件一致，原生网格、类型、EPSG:4326、实际外边缘和 NoData 均正确，18 个原始样本及缺测旗标逐项相同。当前三件各一次尝试即成功。基线在私有 `.verification/next-ten-products-20261014/ui/native-six-before/`；六件合并检查期间全局 11 个任务的数量和 ID 集合不变，且本组六件最终 jobs / projects 内容不变，未将其他在途任务的进度误当成读取修改。

三张原生 PNG 均可解码并已目视检查。Geoid 连续纹理清楚；Hydro ACC / ACA 在原生整幅缩略图上只有 1,256 / 589,824 个非透明像素、5 个灰阶，接近全黑。原因是原分幅绝大多数为海洋 NoData，加上原生默认整幅极值 stretch；这只能证明原生解码与掩膜正确，不代表整幅缩略图已具有良好辨识效果。局部地图使用专用显示范围的实际 Explore UI 回执由主任务另行记录，原值不因显示方式改变。

主任务安全切换到修复冻结构建后，本组六件合并 GET 检查再次与旧基线逐项比较：完整原件、jobs / projects / immutable snapshots、原生检查、36 个原值与六张 PNG 字节全部相同，证明本批其他产品的解码修复没有改变这些原件或成果。新基线在私有 `ui/native-six-final-before/`。随后主任务用同一冻结程序标准重启，最终 PID 为 188752，程序 SHA256 为 `d53a94ef14b5d25b1328556ae3b3a984ddb274004cff2a4572859cf11576b992`；本组 `ui/native-six-final-after/` 再次 GET 比较全部通过。原始文件完整哈希、上述元数据/原值/PNG 都与最终代码重启前一致。

额外读取全局持久回执，与主任务真实重启前记录比较：全部 11 个 jobs / projects 完整字段、任务 ID 集合、尝试次数、传输方式/续传字节和完整摘要都相同，0 POST、无新增传输；证据为 `ui/native-six-final-after/global-restart-readback.json`。主任务的真实浏览器重开地图与原值回执 `ui/ui-reopen-after.json` 记录 11 项全部通过，包含本组对应三件。上述结论限于实际三件完整文件与所测区域，不扩大成 355 个原件或全部地理区域的完整验收。研究传输、原生读取、浏览器地图和最终发布分别保留证据，不相互冒充；本批未发布生产版本。

独占实现为 `datasets/next_hydro_geoid.rs` 的 `definitions()` 与 `resolve()`，注册由本批主任务完成。`hydrosheds-accumulation` 只准入 `acc` / `aca`，`etopo-geoid` 只准入 `geoid`；日期、国家、命名区域、季节、错层、错产品与没有相交合格文件的查询均拒绝，不使用另一地区、版本或图源兜底。原值数据类型、尺度、偏移与 NoData 元数据在解析时严格核对固定索引。

私有研究证据在 `.verification/next-ten-products-20261014/public-archives/`：355 个实际原件头部、官方索引/HTML/PDF、完整原件/传输记录、全像元独立检查、18 个原值样本、UI 用例和原生验收脚本。代码仓库只保存有限索引与说明，没有纳入科研 TIFF、凭据或临时私有数据。

## 本批不计入的其他候选

- GEBCO / OpenTopography 2023 IceTopo 和 SubIceTopo：官方桶真实各八件。IceTopo 的北半球 0–90°E 文件为 525,344,084 B，可落在既有 512 MiB 上限内；其他七件均超限。本批为控制重复完整测试体量而暂不接入。研究传输启动后仅终止研究脚本自身，保留未完成 partial；该 partial 没有完整 SHA 或成功声明，不能计作完整下载验收，也不把 Ice / SubIce 两个几乎相同区域当两条渠道凑数。
- GEDTM30、ANADEM、EU DTM：官方 OpenTopography 原件分别约 432 GB、71 GB、16 GB，排除当前完整原件下载路径。没有人为生成小 subset 并伪装成发布原件。
- SRTM NUM：当前实际 OpenTopography 公共桶根目录未找到对应产品，未接入。公共桶缺少不代表全世界无其他官方渠道。
- GHSL BUILT-S / BUILT-V / POP 官方 AWS 数值原件在体量内，本组已将原始目录研究交给主任务另行核对与实现；不在本组两个产品中重复计数。

355 个头部范围查询不扩大科学真实性、网络稳定性、所有文件完整下载、实际 UI、全平台授权或生产发布的结论。
