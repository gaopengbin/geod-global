# MODIS 8 天反射率 COG 接入与验收

2026-10-02。新增 **MODIS · Planetary Computer** 入口，固定 `modis-09A1-061`，包含 Terra MOD09A1 和 Aqua MYD09A1 Collection 6.1。无需 Earthdata 账号即可搜索和下载 Microsoft 分发的三波段 COG；NASA / Copernicus 的受保护产品继续使用[桌面授权入口](provider-accounts.md)，没有真实账号正向验收证据。

## 已接入的流程

探索页按区域和 UTC 日期区间查询公开目录，保留分页及多景选择。文件身份包含卫星、合成起始年积日、水平 / 垂直瓦片、产品版本与生产时间。目录时段、平台、瓦片及三份完整无签名 COG 路径必须一致；工程恢复根据固定产品 ID 恢复合成时段，不把任务下载时间当成观测时间。

MODIS 是逐像元从 8 天时段内选择观测值的合成产品，年末时段可能短于 8 天。界面显示合成时段、标称 500 米和正弦投影，不显示整景云量筛选。目录给出的 `eo:cloud_cover=0` 不作为“没有云”的证据。[产品集合](https://planetarycomputer.microsoft.com/api/stac/v1/collections/modis-09A1-061)

工程下载红、绿、蓝三份 Int16 单波段 COG，依次为 `sur_refl_b01`、`sur_refl_b04`、`sur_refl_b03`。原生端重新检查官方目录的完整资产路径、数据类型、标称分辨率和转换参数，按 `modiseuwest / modis-061-cogs` 容器共享短期只读 SAS。签名仅存在内存，不写入任务、工程、来源记录或诊断。

下载完成后，可查看单波段灰度、本地真彩色组合、原始 DN 与反射率。缩略图使用已有持久化缓存，重启后重新验证文件身份并复用。RGB 组合重新校验三份完整原文件的托管路径、大小及 SHA-256，并要求完全相同的网格；显示预览使用最近邻采样和共同有效像元的 2–98 百分位拉伸。

## 科学参数与格式边界

实际三份文件均为 2400 × 2400、单波段 Int16、PixelIsArea。文件中像元间距约 463.312716528 米；“500 米”是产品标称分辨率，不能覆盖实际网格间距。反射率为 `DN × 0.0001`，NoData 为 −28672；有效 0、负数以及超过 1 的反射率保持原值。NoData 透明，不自动做云、阴影或质量掩膜。

使用 NASA MODLAND 正弦投影，参考球半径 6371007.181 米，中央经线、假东和假北均为 0。原生端验证 GeoTIFF 的投影方法、球半径、单位和瓦片网格，应用以 `MODIS:Sinusoidal` 作为固定投影标识，不伪造 EPSG 编号。[NASA 网格参数](https://modis-land.gsfc.nasa.gov/GCTP.html)

下载文件是 **Planetary Computer 从 NASA 产品转换的 COG**，不是 NASA 原始 HDF。来源署名和详情保留此区别，并提供 [LP DAAC 引用与数据政策](https://lpdaac.usgs.gov/data/data-citation-and-policies/)。本页的三份 Int16 反射率原件不包含 QA、NIR 或其他反射率波段。QA 文件已另行接入并记录[独立验收](modis-quality.md)，完整 HDF 尚未接入。

## 实际验收

通过应用原生任务服务下载 `MYD09A1.A2025177.h08v05.061.2025189031924` 的三份完整 COG，合成时段为 2025-06-26 至 2025-07-03，总计 33,646,642 字节。

| 波段 | 字节数 | SHA-256 |
|---|---:|---|
| 红 / b01 | 11,417,054 | `09373ed0cc2e1c312947061620d86dea065277138e3922be820458ed77668dc3` |
| 绿 / b04 | 11,245,921 | `37e64cb2e0f3a092d176ebe24642ee9b044b6d06c24bf5785f9556d4e35df229` |
| 蓝 / b03 | 10,983,667 | `d535688c0ba752e52919f990339f33a0da412b4eaa8b9322b61310810ec412da` |

独立 Rasterio / GDAL、NumPy 和 Pillow 核对全部 589,824 个 RGB 预览像元（1,769,472 个通道值）、24 个全分辨率通道 DN 及反射率、原始 SHA-256、正弦投影与网格。原生运行时重启后，工程时段、RGB 预览、源文件校验值与三个持久化缩略图保持一致。缓存条目内容不变，访问时间正常更新。

实际公开目录的探索页通过英文浅色、中文深色验证；文件到 RGB 地图、原值检查与图层详情通过中文 / 英文、浅色 / 深色及 1024 / 1440 像素视口验证。测试使用无界面浏览器和隔离原生服务，没有操作用户桌面。摘要见[验收记录](../prototype/qa/modis-verification.json)；目录回归输入见[实际目录快照](../prototype/qa/modis-catalog-item.json)。

## 仍需推进

后续已接入 MODIS 原始单波段的工程拼接、矩形和带孔洞多边形裁剪；红波段三种成果已用两个实际文件独立核对全部 2,193,265 个输出 DN，成果地图、缓存与重启恢复也已验收，见[工程处理](modis-processing.md)。本页机器记录保留此前原始文件与 RGB 接入阶段的状态；最新处理证据以该文档的验收记录为准。

后续已接入[单波段工程处理](modis-processing.md)、派生 RGB、[科学 RGB 文件](scientific-rgb.md)及[QC / State 质量层](modis-quality.md)。重投影、NASA 原始 HDF、自动质量掩膜和其他 MODIS 产品仍待接入；VIIRS 的独立状态见[处理说明](viirs-processing.md)。
