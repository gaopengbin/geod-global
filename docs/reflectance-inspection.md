# Landsat / HLS 原始波段检查

2026-10-01，下载完成的 Landsat Collection 2 Level-2 和 HLS L30 v2.0 红、绿、蓝单波段可以在「我的数据」显示本地缩略图、检查栅格，并打开工作空间读取原始像元。应用使用内置 Rust TIFF 解码器，不依赖用户安装 GDAL 或 Python。

| 产品 | 源类型 | 反射率换算 | NoData |
|---|---|---|---|
| Landsat 8/9 Collection 2 Level-2 | UInt16 | `DN × 0.0000275 - 0.2` | 0 |
| NASA HLS L30 v2.0 | Int16 | `DN × 0.0001` | -9999 |

转换仅用于经审核的版本、主机、产品 ID 与波段路径。实际文件的采样类型、NoData 和 30 米网格必须匹配该产品；缺失或冲突的 GeoTIFF 空间标签会拒绝读取。参数不会由任意灰度文件或用户输入推测。

每次检查或像元请求都校验受管文件路径、实际字节数及 SHA-256，后续解码使用同一份已校验字节快照。单文件最大 512 MiB、单边最大 20000 像元；一次只解码所需的条带或瓦片，解码缓冲有独立的 32 MiB 限制，读取有处理期限。原文件不被修改。

## 显示与取值

本轮提供单波段灰度显示，未合成为真彩色影像。预览在原始分辨率上按最近邻选取不超过 768 × 768 个样本，排除 NoData 后，以采样值的 2–98 百分位拉伸为灰度。有效样本数量与 DN 拉伸范围显示在检查窗口；它们是显示采样统计，不能当作整景统计。全为 NoData 的预览透明，常量有效值显示为中灰。缩略图最多 160 × 160，按自身样本计算显示范围，复用现有有容量限制的持久化缓存。

工作空间像元请求返回全分辨率文件中的 DN、行列号、坐标与反射率，独立于灰度图取色。反射率保留负值和大于 1 的值，不以显示范围截断；NoData 不生成反射率。当前未下载或应用 QA / Fmask 质量掩膜，也不将 HLS 当作 Sentinel-2 SCL 分类图。

空间信息支持 WGS84 UTM 北 / 南、未旋转的 30 米网格。PixelIsArea 的标签使用像元边界；PixelIsPoint 的标签使用像元中心，读取时换算为外边界。实测 Landsat 使用 PixelIsPoint，其标签 `(462300, 4264200)` 对应左上像元中心；外边界起点为 `(462285, 4264215)`。此处理与 [GDAL RFC 33](https://gdal.org/en/stable/development/rfc/rfc33_gtiff_pixelispoint.html)一致，避免 15 米定位偏移。

## 验收范围

[机器验收记录](../prototype/qa/reflectance-inspection-verification.json)区分实际文件、协议夹具和界面证据：

- 原有隔离工程的 Landsat 9 三个原始波段已实际下载，共 281,060,423 字节，各为 7671 × 7791、UInt16、EPSG:32610。未为本轮重复下载。
- Rasterio/GDAL 独立核对三个文件的 CRS、像元大小、NoData 和全部外边界；24 个原始像元与反射率一致。
- NumPy/Pillow 独立重建每幅 756 × 768 预览，全部 580,608 个 RGBA 像素逐一匹配。
- 运行时与前端检查 HLS Int16 夹具，覆盖负 DN、NoData 和大于 1 的反射率；这不等于 NASA 真实账号原文件验收。
- 实际本地运行时响应用于明暗主题与紧凑窗口的文件卡片、检查窗口、工作空间和像元查询验收。仅探索目录使用固定重放，与真实原始文件证据分开。

Landsat / HLS 原始波段的工程裁剪与同网格拼接已接入，输出保留原 DN、NoData 和转换参数；见[工程处理及独立验收](reflectance-processing.md)。同一景三个原始波段的[本地 RGB 组合](local-rgb.md)已接入，Landsat 使用完整真实文件验收，HLS 使用有符号夹具验证。跨 CRS 处理、RGB 合成文件导出、派生波段组合及质量掩膜尚未接入。NASA 和 Copernicus 的授权入口已经可用，成功授权与受保护原产品下载仍需真实账号验收。没有生成安装包或公开发布。

官方参数参考：[USGS Collection 2 Level-2](https://www.usgs.gov/landsat-missions/landsat-collection-2-level-2-science-products)、[NASA HLS v2 用户指南](https://lpdaac.usgs.gov/documents/1698/HLS_User_Guide_V2.pdf)。
