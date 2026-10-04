# MODIS 质量层与像元状态

2026-10-04。MOD09A1 / MYD09A1 Collection 6.1 的两份原始质量 COG 已接入工程下载、本地地图、原值解码和持久化缩略图。来源为 Microsoft Planetary Computer 转换分发的 COG，不是 NASA 原始 HDF。[NASA 产品说明](https://www.earthdata.nasa.gov/data/catalog/lpcloud-mod09a1-061)

## 使用

在 MODIS 下载内容中选择单独的质量文件，或选择「RGB 波段 + 质量层 + 像元状态」保存五份文件到同一工程。旧 RGB 工程继续选景时可以补充质量资产，原有场景、时段、RGB 路径及转换参数保持原记录。五类资产按三个反射率波段、两个质量层排列。

在工程文件中打开质量层，地图显示选定质量字段的分类预览；点击像元后展开「解码质量标记」查看原始无符号数、十六进制、二进制及各位字段。缩略图来自受管原文件，与原文件 SHA-256 绑定，重启后可复用；原文件被修改时拒绝旧缓存。

| 工程资产 | 原始数据层 | 数据类型 | 填充值 |
|---|---|---|---|
| `modis_qc` | `sur_refl_qc_500m` | UInt32 | 4294967295 |
| `modis_state` | `sur_refl_state_500m` | UInt16 | 65535 |

质量层保留完整原始位值，不使用反射率 scale / offset。QC 预览使用 MODLAND 两位产品质量，状态预览使用两位云状态。图例统计覆盖当前文件全部有效像元，并单独列出 NoData；不代表云覆盖率。原件的统计为完整瓦片，工程裁剪结果的统计为该结果文件。

解码依据 [NASA MOD09 C61 用户指南](https://landweb.modaps.eosdis.nasa.gov/data/userguide/MOD09_User_Guide_V61.pdf)的表 10、13：包括七个波段的质量、大气 / 邻近校正、云影、陆水类型、气溶胶校正不确定性、卷云、内部云 / 火点、雪冰、邻云和盐滩等字段。C61 状态位 14 为盐滩，位 15 为内部雪检测。未设置的云状态保留“产品默认晴空”含义；未定义的波段质量代码明确标为未定义。填充值不解码为有效标记。

## 实际文件验收

原始公开条目为 [`MYD09A1.A2025177.h08v05.061.2025189031924`](https://planetarycomputer.microsoft.com/api/stac/v1/collections/modis-09A1-061/items/MYD09A1.A2025177.h08v05.061.2025189031924)，时段 2025-06-26 至 2025-07-03。两份文件均通过实际原生工程下载流程获取，原件保留在隔离验收目录。网格为 2400 × 2400、PixelIsArea、MODIS 正弦投影；文件实际像元间距约 463.3127165 米，产品标称 500 米。

| 数据层 | 实际字节数 | SHA-256 |
|---|---:|---|
| QC | 7,013,560 | `e735aa1b520ff09f4fd9a65c08f49dedac2c6a75e39bc49bf982152849a6f7a7` |
| State | 1,800,792 | `b5fb227c174b205802c176b20d429870cfea5eeb1aca256b871ec047c924ffa2` |

独立 Rasterio / GDAL 读取核对了两份原件的全部 11,520,000 个像元分类统计、456 个原始取值与对应位字段、1,230,848 个预览及缩略图 RGBA 像素、源投影 / 网格 / NoData 和完整哈希。此瓦片没有填充值，QC 最大有效值未设置最高位；32 位最高位、未定义质量码及填充值语义另以明确的合成文件测试覆盖。

断网代理下重启后，工程及补充的质量资产恢复，原始检查和缩略图完全一致；修改验收原件后检查和缓存均拒绝，恢复原件后再次通过。实际 stdio MCP 在直接只读与本地服务只读模式下保留完整无符号值和全部字段；服务断开时不自动回退。三种窗口宽度、两种语言 / 主题的实际构建界面通过文件显示、地图绘制、像元读取、缓存、卡片高度和精确桌面 CSP 检查。

状态文件首次下载出现上游连续 45 秒无数据的失败，后续工程下载成功。任务列表仍保留该失败记录；同一原件的成功文件不再让旧失败占据工程待处理区。完整摘要及来源见[验收记录](../prototype/qa/modis-quality-verification.json)。

## 边界与复现

本页保留原始质量层查看与解码的首次证据。随后质量层自身的工程裁剪 / 拼接也已接入，五类实际文件及处理专项验收见[工程质量层处理](modis-quality-processing.md)。查看质量层不改变 RGB / 反射率；生成科学 RGB 时，可显式选择[同景 QC / State 筛选](modis-rgb-quality-mask.md)或[多景联合质量选景](modis-coupled-rgb.md)。多景处理重新读取同景五份原件，不直接套用独立拼接的 QA 值。其他 MODIS 产品、NASA 原始 HDF 和更通用的策略尚未接入。这些公开 COG 通过现有 PC 临时只读授权访问，不建立 NASA / Copernicus 账号授权或受保护原产品的正向下载证据。

界面验收使用无头 Edge 与真实原生任务服务，未操作用户桌面；桌面程序已重新构建，安装版 WebView / 窗口交互仍需人工验收，不据此宣称发布完成。

在仓库根目录运行以下脚本，所有写入限指定的 `.verification/modis-quality-*` 目录；同一目录的服务和直接 MCP 不要同时打开。原生可执行文件需先构建，Python 需安装 Rasterio、NumPy、Pillow。

```text
node scripts/verify-modis-quality.mjs .verification/modis-quality-YYYYMMDD 4601
python -X utf8 scripts/verify-modis-quality.py .verification/modis-quality-YYYYMMDD
node scripts/verify-modis-quality-ui.mjs .verification/modis-quality-YYYYMMDD 4601 4602
python -X utf8 scripts/verify-modis-quality-mcp.py .verification/modis-quality-YYYYMMDD --port 4601
```

原生文件读取、填充值、无符号最高位、错误源 / SHA / 类型 / 网格、完整计数及旧工程恢复由运行时测试覆盖；界面测试覆盖五文件选择、质量图例、中文字段和已成功原件的失败历史处理。该证据限本页具体产品及文件，不外推为平台所有产品完成。
