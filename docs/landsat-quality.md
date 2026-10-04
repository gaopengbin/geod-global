# Landsat 原始质量文件

2026-10-04。本页记录 Landsat 8/9 Collection 2 Level-2 的首次 `QA_PIXEL` 与 `QA_RADSAT` 原文件下载、本地地图、原始 UInt16 取值、位字段解读、完整统计、持久化缩略图及只读 MCP 验收。当时实际原文件来自一景 Landsat 9；后续补齐的真实 Landsat 8 文件及质量工程处理另见[质量工程处理](landsat-quality-processing.md)，两组证据分别保存。

## 软件内使用

在探索页选择 Landsat 8/9，下载内容可选「RGB 波段 + 像元质量 + 饱和度 · 5 个文件」，或分别选择两种质量文件；默认 RGB 下载内容保持原有三个波段。工程详情显示各文件的下载进度，文件卡片可打开工作空间，按源网格查看质量图例、读取原始像元和展开统计。

原有工程继续选景时，实时目录可补入同一原始处理目录的质量文件，不改动已保存的 RGB 地址、参数或日期。若新的目录条目指向另一处理版本，不会把它的 QA 与旧 RGB 混用。工程和任务保存无签名原始地址；原生下载核对当前官方条目，再使用现有只读 SAS 和用户代理。

## 质量含义

规则来自 [USGS Collection 2 Quality Assessment Bands](https://www.usgs.gov/landsat-missions/landsat-collection-2-quality-assessment-bands)。两层保存完整的 16 位无符号原值，不套用反射率 scale/offset，也不把位字段当作 SCL 分类编号。

- `QA_PIXEL` 第 0 位表示填充；判断覆盖时检查这一位，包括其他位同时置位的值。其余字段分别描述云扩张、卷云、云、云影、雪冰、水和置信度。第 6 位 clear 仅描述云和扩张云标志，不意味着像元已通过完整质量筛选。
- `QA_RADSAT` 原值 0 表示没有标记饱和或地形遮挡，仍保留为有效原值。这一层本身无法确认影像覆盖，须结合匹配的 `QA_PIXEL` 填充标志。B2/B3/B4 对应 RGB；其他波段饱和也单独保留，不推定 RGB 同时无效。
- 置信度中的保留编码、未使用位和完整十六进制 / 二进制原值均保留；不把未知编码解释成安全像元。统计包含全部原始像元，也包括填充与保留编码。

地图颜色按显示优先级分组，同一像元只显示一种颜色；一个像元可同时具有多个原始标志。因此图例分类数量不是全部独立标志数量，也不能直接当作整景云量。展开「完整分辨率标志统计」可查看各位字段的独立分布。

TIFF 的 NoData 标签与产品的填充语义分别保存。实测两份原文件没有 TIFF NoData 标签；`QA_PIXEL` 仍按第 0 位透明显示填充，`QA_RADSAT` 的零值仍显示。读取同时支持 PixelIsArea / PixelIsPoint；Point 网格仅转换显示外边界的半像元，保留原始样本中心。

## 实际验收

[机器记录](../prototype/qa/landsat-quality-verification.json)绑定原生、独立解码、MCP 和界面四份通过记录，以及按 SHA-256 保存的完整证据快照。官方条目为 `LC09_L2SP_044034_20250628_02_T1`，原始处理目录为 `LC09_L2SP_044034_20250628_20250629_02_T1`。

| 原文件 | 字节 | 网格 | 完整统计 |
|---|---:|---|---|
| QA_PIXEL | 986,984 | 7671 × 7791、UInt16、EPSG:32610、30 米、PixelIsPoint | 全部 59,764,761 个像元；非填充 40,717,367，填充 19,047,394 |
| QA_RADSAT | 224,657 | 同一原始网格 | 全部 59,764,761 个无符号值；零值保留，不据此声明覆盖完整 |

两份原文件共 1,211,641 字节，由本轮私有目录中的原生 worker 实际取得，之后修复 Point 网格读取与验收脚本时复用已完成原件，没有重复冒充新下载。首次失败记录保留为诊断，不计入通过范围。RGB 原件沿用已有独立验收；本轮实际下载验收只有这两个 QA 文件。

Rasterio 1.3.9 / GDAL 3.6.4 独立比较全部 119,529,522 个原始质量样本统计、各字段分布、网格与样本中心；两张全预览的 1,161,216 个 RGBA 像素、两张缩略图的 50,240 个 RGBA 像素和 22 个覆盖实际标志的原值点全部一致。真实样本含云、卷云、云影、水及饱和标志；雪冰、地形遮挡和非零保留 / 未使用位没有真实正例，这些编码和高位读取另以明确的合成回归验证。

直接模式与本地服务模式只读 MCP 共 44 次原值查询通过，完整统计与字段保留一致，预览图不进入 MCP 文本；本地服务断开后明确拒绝查询。生产界面在 1440 英文浅色、1024 中文深色、900 中文深色三种视口通过，比较 1,741,824 个源预览 RGBA 像素、实际地图绘制、中心原值、字段解读和 74 个已显示的统计项；没有外部网络请求、页面 / CSP 异常或横向溢出。

运行时设置不可达代理后重启，两张缩略图的字节、文件身份和创建时间保持一致，原始检查和像元仍可离线读取。缓存命中可更新用于 LRU 的修改时间，不要求修改时间保持不变。修改原件会拒绝失效缓存；损坏 JSON 条目可重新生成。详见[缩略图缓存](thumbnail-cache.md)。

原生 Landsat 相关回归 9 项、已有 MODIS 回归 9 项和前端相关逻辑 48 项通过，格式、严格运行时 Clippy、仓库隔离、契约和配方检查通过。三种尺寸的截图另经实际查看，位字段和值使用统一的对齐与主题颜色。

## 开发程序与复现

本次原生验收程序为 `.verification/landsat-quality-20261004-point/runtime-33abd762af59f7fe.exe`，50,490,368 字节，SHA-256 为 `33abd762af59f7feed544f20b0e1bd0f41d351cd9173f7ac0d0fcb73b644e454`。

带本地前端资源的开发程序为 `.verification/renderer-landsat-quality-20261004/desktop-65d5eea72d30fe07.exe`，80,616,448 字节，SHA-256 为 `65d5eea72d30fe07f4887aacbded9a16eb5a683b1e96f500ce8ffd03b194335a`。当前 renderer 的全部 662 个文件与资源校验值一并冻结；此前 MODIS 多景开发程序与 661 个资源的验收快照保留不变。

下列验证在隔离目录执行，依次运行，避免两个服务同时操作同一数据目录。首项须使用尚不存在的新目录，实际取得两份完整原件；后两项复用它的已通过记录。

```powershell
rtk proxy node scripts/verify-landsat-quality.mjs .verification/landsat-quality-new 4635 .verification/landsat-quality-20261004-point/runtime-33abd762af59f7fe.exe
rtk proxy python -X utf8 scripts/verify-landsat-quality-mcp.py .verification/landsat-quality-new --port 4635
rtk proxy node scripts/verify-landsat-quality-ui.mjs .verification/landsat-quality-new
```

[汇总脚本](../scripts/summarize-landsat-quality.py)只在所有记录相互绑定且通过、原件哈希一致、renderer / 开发程序未变化和历史 MODIS 快照完好时写出公开验收摘要。

## 尚未接入

上述历史验收提供原始质量查看，不包含工程处理。后续的 Landsat QA 裁剪 / 拼接与真实 Landsat 8 文件另在[质量工程处理](landsat-quality-processing.md)完成独立验收，保持本页原始证据的范围。后续 Landsat 9 同景原件 / 单景工程成果的[科学 RGB 质量筛选](landsat-rgb-quality-mask.md)另行通过真实验收，不改变本页历史范围。Landsat 8 RGB、多景联合质量选择、气溶胶或热红外质量层及通用重投影仍待验收 / 接入。单文件上限沿用 512 MiB；原始 UInt16 解码有网格、分块与时限约束，不读取概览层冒充完整统计。

界面验收使用隐藏浏览器、生产资源、桌面 CSP 和真实原生接口桥接；尚未替代原生 WebView 窗口验收。本轮没有制作安装包或远程发布。NASA / Copernicus 的[软件内授权入口](provider-accounts.md)已可用，没有账号时继续保留受保护生产原件的正向验收为待完成。
