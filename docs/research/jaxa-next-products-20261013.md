# JAXA 新独立数值产品核查：日本土地覆盖与短波辐射

核查日期：2026-10-13。本记录区分真实发布文件、独立原件验收与软件接入验收；不涉及新增账户、密钥或公开发布。

此次从实际 [JAXA Earth STAC 根目录](https://data.earth.jaxa.jp/stac/cog/v1/catalog.json) 检索，选择与已接入的土壤水分、地表温度、NDVI、森林分类、叶绿素及降水率不同的两个产品。均使用 JAXA 正式转换并公开发布的数值 COG，未把渲染图、裁剪结果或浏览器截图视为原始数值文件。

| 产品 / 软件 ID | 有限接入范围 | 实际原始文件 |
|---|---|---|
| HRLULC 日本土地覆盖概览 / `jaxa-hrlulc-japan` | v25.04、2024 年、COG 第 1 级，一个东北 90° 分块 | 66,941 字节；3600×3600 UInt8；EPSG:4326、0.025° |
| Terra MODIS 短波辐射 / `jaxa-modis-swr` | v811、2026-09 发布、COG 第 1 级，8 个 Item；独立验收东北原件 | 6,907,674 字节；1800×1800 UInt16；EPSG:4326、0.05° |

## 日本土地覆盖：分类概览与原始 10 米产品分开

实际 [Collection](https://s3.ap-northeast-1.wasabisys.com/je-pds/cog/v1/JAXA.EORC_ALOS_HRLULC.v25.04_japan/collection.json) 的 ID 与 [2024 年原始 Item](https://s3.ap-northeast-1.wasabisys.com/je-pds/cog/v1/JAXA.EORC_ALOS_HRLULC.v25.04_japan/2024/1/E090.00-E180.00/N00.00-N90.00.json) 完全匹配。Item 为 STAC 1.1，使用 `proj:code=EPSG:4326`，明确第 1 级为 overview。实际 [完整 COG](https://s3.ap-northeast-1.wasabisys.com/je-pds/cog/v1/JAXA.EORC_ALOS_HRLULC.v25.04_japan/2024/1/E090.00-E180.00/E090.00-N00.00-E180.00-N90.00-HRLULC.tiff) 的网格间距是 0.025°，约为赤道 2.8 千米，不能称为 10 米产品或传感器独立分辨率。

[官方 v25.04 产品说明](https://www.eorc.jaxa.jp/ALOS/en/dataset/lulc/lulc_v2504_e.htm) 描述 2020、2022、2024 三个年度的日本分类，原始高分辨率产品按 1°、12000×12000 的约 10 米文件发布，每年 131 份，使用多卫星和辅助数据分类。其原件下载流程要求注册；本接入只读取 JAXA Earth 实际公开的 overview 链接，不创建账户、不获取或替换这一独立原件流程。

完整独立解码 12,960,000 个像元，212,366 个有效、12,747,634 个 NoData，无未知类别。15 类均实际出现：1 水体、2 建成区、3 水田、4 耕地、5 草地、6 落叶阔叶林、7 落叶针叶林、8 常绿阔叶林、9 常绿针叶林、10 裸地、11 竹林、12 光伏板、13 湿地、14 温室、15 岩礁与潮滩。0 为 NoData，比例 1、偏移 0；类别不是树冠覆盖百分比、辐射或连续指数。

- 完整 SHA-256：`5e2ba21ad538ffc192bfd9ed20ca339a29056a6c1dc524ef532e0b6e79eaca37`。
- 原 Item 时间：`2024-01-01T00:00:00+00:00` 至 `2024-12-31T23:59:59+00:00`；TIFF XML 含相同年度范围。产品版本于 2025 年 4 月发布，此 COG 于 2026 年 5 月生成，都不是观测年。
- 地图请求必须与 Collection 日本包络 `[122,20,154,46]` 相交；东北矩形文件 `[90,0,180,90]` 的交集不证明日本有效像元覆盖。北京像元实测为 0 NoData，解析器对北京 AOI 明确拒绝，不代入日本演示区域。
- 东京区域样本：列 1980、行 2180，原值 2（建成区）。列 1505、行 2552 原值 15（岩礁与潮滩）；列 0、行 0 为 NoData。
- 原 TIFF 内部 overview 2×、4×，`OVV_RESAMPLING=MODE`；原件及类别未合并、平均、重采样或重新投影。灰度预览不是原始分类配色图例。

## 短波辐射：保留月目录与半月观测声明的冲突

实际 [Terra MODIS SWR Collection](https://s3.ap-northeast-1.wasabisys.com/je-pds/cog/v1/JAXA.JASMES_Terra.MODIS_swr.v811_global_monthly/collection.json) 与 [东北 Item](https://s3.ap-northeast-1.wasabisys.com/je-pds/cog/v1/JAXA.JASMES_Terra.MODIS_swr.v811_global_monthly/2026-09/1/E090.00-E180.00/N00.00-N90.00.json) 公开提供完整 [数值 COG](https://s3.ap-northeast-1.wasabisys.com/je-pds/cog/v1/JAXA.JASMES_Terra.MODIS_swr.v811_global_monthly/2026-09/1/E090.00-E180.00/E090.00-N00.00-E180.00-N90.00-swr.tiff)。[JASMES 产品表](https://kuroshio.eorc.jaxa.jp/JASMES/datalist.html) 将 `swr` 定义为短波辐射；[历史算法与格式说明](https://suzaku.eorc.jaxa.jp/GLI/data/final/landpar/index.html) 介绍地表太阳辐射估计，单位 W/m²。历史版本的算法细节不能直接替代此 v811 文件的实际声明，校准采用原始 Item 的精确字段。

Collection 标题及 `YYYY-MM` 路径称月度产品，但全部 8 个实际 Item 以及完整 TIFF XML 只标注 **2026-09-01T00:00:00.000000Z 至 2026-09-15T23:59:59.999999Z**。接入标题为“短波辐射”，保留“2026-09 发布”限定和真实时间范围；不补为月末，不宣称验证了完整 9 月平均值。

完整独立解码 3,240,000 个像元，3,168,913 个有效、71,087 个 NoData，无未知值。保留 UInt16 DN；原 Item 声明物理短波辐射 `[W/m²]=DN×0.01`，偏移 0，65535 为 NoData / error，0 是有效声明值。实际东北原件没有零有效样本；观测有效 DN 范围 2463–31491。TIFF 嵌入比例 1、偏移 0；不自动把 DN 改写成物理值，也不积分成能量或推断光伏发电量。

- 完整 SHA-256：`7d1ec2222c596a7ffabee3476be365d1684125a101e20bcc8f65a402c12b8762`。
- 北京样本：列 528、行 1002，原值 23495，按发布方比例解释为 234.95 W/m²；东京样本原值 11661，解释为 116.61 W/m²。
- 列 0、行 0 为 65535 NoData。原件内部 overview 2×，`OVV_RESAMPLING=MODE`。地图灰度不代表科学辐射配色或空间/时间平均方法。
- Item 使用完整产品 ID，而 Collection ID 为通用 `collection`。完整原始 JSON 保留冲突，不伪造别名或声称匹配成功。

## 来源、许可与软件接入边界

两产品实际 Item / Collection 都声明 `proprietary`，链接 [JAXA 研究数据政策](https://earth.jaxa.jp/en/data/policy/)；该政策允许保留来源及第三方署名后的免费使用，并要求商业使用事先通知 JAXA。[JASMES 署名说明](https://kuroshio.eorc.jaxa.jp/JASMES/registration.html) 对 MODIS 全球气候产品保留 JAXA / NASA / GSFC 来源。公开无账号分发不等于 Creative Commons 许可；本次不代用户发送通知。

两个实际 Item 均未声明文件校验和。传输回执中的本地 SHA-256、MD5 和 HTTP ETag 单独保留，不声称完成发布方 checksum 验证。完整传输核对 HTTP 长度、TIFF 魔数及 512 MiB 限制；解码读取全像元，不读取渲染图替代数值原件。所有实际 Item / Collection 及科学字段作为不可变文档归档。

研究证据保存在 `.verification/jaxa-next-products-20261013/`：原始 JSON / TIFF、响应头回执、每产品独立解码与 6 个原值样本、完整分类直方图、官方页面和校验回执。新增 `datasets/jaxa_extra.rs` 及两份不可变 JSON，仅覆盖上述两个产品；包括 AOI、时间、类别、数值校准、NoData、来源篡改和 Collection 身份边界测试。共享注册、前端目录及桌面界面由本轮主任务验收，不根据研究记录推断 UI 已通过。

随后已完成两个产品的真实原生闭环验收，固定 runtime SHA-256 为 `92843d51bb9801edba37a51da39ffc3c38cdbd11a5bb58c871bfe263c85d3962`。每个产品使用新 UUID 私有存储与新下载任务，完整原件的大小和 SHA-256 均与独立下载一致；实际原生网格、每产品 6 个原值 / NoData 样本、768×768 PNG 和重启后项目、原件、检查及像元重新打开全部通过。未复用用户数据库、旧任务或已有原件。汇总记录为 `native-acceptance.json`。

首次 SWR 原生连接出现明显网络波动：180 秒测试等待上限到达时仍在运行，已收到 4,471,743 / 6,907,674 字节，runtime 未报错；这次状态和部分传输证据已保留。随后仅对 SWR 使用全新的存储及新完整下载任务，将验收等待上限扩为 900 秒，实际约 3.93 秒完成原件传输，并完成后续原值与重开检查。没有把首次超时当作验收成功，也没有借用首次部分文件；这一波动说明本次观测速率不能当作稳定下载速度保证。
