# 科学三波段 RGB GeoTIFF

2026-10-04。工作空间中的本地 RGB 组合可生成一份可独立使用的三波段 GeoTIFF。默认保留原始 16 位 DN、每个波段的 scale / offset、NoData 和源像元网格；显示预览的拉伸不会写入科学数值。工程区域的红、绿、蓝单波段成果也可按同一规则组合。原始不筛选路径的机器证据见[验收记录](../prototype/qa/scientific-rgb-verification.json)。MODIS 同景文件可选[质量筛选](modis-rgb-quality-mask.md)，多景可选[联合质量选景](modis-coupled-rgb.md)；Landsat 同景原件与匹配单景工程层可选[QA_PIXEL / QA_RADSAT 筛选](landsat-rgb-quality-mask.md)，匹配的多景工程层可选[整组三通道质量选景](landsat-coupled-rgb.md)，包含真实 Landsat 8 整景和 Landsat 8/9 多景验收。合格 DN 不变，无合格候选时写为 NoData，产品规则随成果保存。

## 软件操作

1. 下载同一景的红、绿、蓝三个波段；VIIRS 先在工程内准备 M5 / M4 / M3。
2. 打开工作空间中的本地 RGB。组合图层的图标栏提供「生成科学 RGB」，悬停说明其用途。
3. 应用实际核对三个文件的网格、类型、标定参数、校验值和所需磁盘空间。输入成果名称后提交。
4. 在任务或所属工程查看后台处理进度。成功后可从文件卡片打开成果、读取原始 DN / 反射率、查看来源或生成交付包。

合成工程成果时，先分别生成红、绿、蓝单波段，再打开对应的本地 RGB。三份成果必须属于同一工程、区域、多边形、输入场景顺序和网格；不自动凑齐缺少的波段、不隐式调整范围。成果名称保留在任务与文件列表中。

成功提交的 RGB 文件拥有自己的 SHA-256、来源清单及持久化缩略图。重启、移走父级单波段文件后，读取成果和生成交付包仍直接使用 RGB 文件；重新生成或重试未完成的任务仍需校验其来源。

## 数据与处理边界

支持已审核的 Landsat 8/9 C2 L2、HLS L30 v2.0、MOD/MYD09A1 v061 和 VIIRS 09A1 v002。输入是三个已完成的托管原始、准备或工程处理任务，固定 red / green / blue 顺序。Landsat 为 UInt16，其他为 Int16。不筛选时，每个通道的原始数值独立保留，包括有效零、负数、超过 1 的反射率和某个通道为 NoData 而其他通道有效的情况。产品对应的质量筛选在合格像元中同样保留这些数值，并将 QA 剔除像元的三个通道一同设为 NoData。

三个文件必须拥有相同产品、校准、网格与采样解释。原始场景还必须来自同一产品目录；不同 Landsat 处理批次不能混用。VIIRS 准备层绑定同一原始 HDF5。工程处理层绑定各自经过验证的来源参数，不以名称或尺寸相同代替来源匹配。

逐个核对完整文件大小、SHA-256、托管路径和 GeoTIFF 标签。逐块读取原始快照，写入有界临时平面，再按 64 行条带写出 Deflate GeoTIFF；原始块与临时平面逐项核对，完整解码输出并核对所有通道样本后才提交成功文件和来源清单。取消、磁盘不足或校验失败不提交成功成果；中断任务支持恢复记录和从头重试。运行软件无需外部 GDAL 或 Python。

保留经过校验的源 CRS GeoKeys、参数和名称标签，包括 MODIS 自定义球体的引用名称；输出 Point / Area 标签与实际网格一致。位置仍按源网格写入，没有重采样、重投影、平均或色彩值量化；默认不应用质量筛选，MODIS 可显式选择 QC / State 规则，Landsat 可显式选择 QA_PIXEL / QA_RADSAT 同景筛选或多景联合选择。GDAL 可读取各通道标定参数、名称和 NoData；遵循 [GeoTIFF 标签](https://docs.ogc.org/is/19-008r4/19-008r4.html)及 [GDAL GeoTIFF 元数据约定](https://gdal.org/en/stable/drivers/raster/gtiff.html)。

未经压缩的 RGB 样本上限为 500 MiB，最终 TIFF 上限为 512 MiB，读取单边上限 20000 像元；预检报告完整临时空间预算，MODIS / Landsat 同景筛选计入两份有界质量平面，多景选择还计入完整的一景五层暂存和输出候选标记。输出是普通 Deflate GeoTIFF，**不是 COG**。任意波段搭配、跨网格处理、其他 QA / 指数计算、单个科学 RGB 的后续裁剪及更大文件仍需另行实现。

## 原值与交付包

地图用有界最近邻显示预览，任一通道 NoData 时显示透明；2–98 百分位拉伸仅用于显示。点击地图从成果的原始分辨率文件读取三个 DN，逐通道返回 NoData 与未截断的反射率。显示 PNG 无法替代科学数据文件。

交付 ZIP 含五个成员：RGB GeoTIFF、来源元数据 JSON、预览 PNG、README 和校验清单。JSON 保留具体产品、源任务、无签名原始 URL、SHA-256、各波段标定、工程处理参数和输出校验值；不包含账户秘密或任意本机来源路径。预览仅用于辨认。重复生成会核对既有 ZIP；既有文件改变时会拒绝复用。

## 实际文件验证

使用此前由本软件原生下载的完整公开文件，在独立验证目录内生成三个成果；本轮没有新增供应商请求。拒绝代理用于验证离线读取，原始工作目录和文件保持不变。Rasterio / GDAL 独立解码全部 DN，并核对 CRS、仿射变换、边界、类型、NoData、通道名称、scale / offset、有效数和逐通道样本摘要。

| 输入 / 输出 | 输出网格 | 样本核对数 | 文件大小 |
|---|---:|---:|---:|
| Landsat 9 `LC09_L2SP_044034_20250628_02_T1` 三个完整原始波段 | 7671 × 7791 × 3 | 179,294,283 | 223,494,366 字节 |
| Aqua `MYD09A1.A2025177.h08v05.061.2025189031924` 三个完整 COG | 2400 × 2400 × 3 | 17,280,000 | 23,413,098 字节 |
| 同一 MODIS 工程的三个区域裁剪成果 | 95 × 39 × 3 | 11,115 | 15,337 字节 |

共 **196,585,398 个通道值全部一致**。三个 MODIS 单波段裁剪还分别与原始 COG 的 `[722, 518, 95, 39]` 窗口核对，共 11,115 个原始 DN 一致。这补充了绿、蓝波段真实单景矩形裁剪证据；不代表其多瓦片或多边形真实验收全部完成。

三个 ZIP 的全部成员 CRC、校验清单、来源和重复导出哈希均通过。移走工程来源成果并实际重启原生服务后，科学 RGB 的独立检查、原值取样与交付包复验通过。有符号负值、有效零、极值、独立通道 NoData、取消、校验值变化、篡改输出和崩溃重试另由明确标注的合成文件回归测试。

实际 stdio MCP 验证包括直接拥有目录和连接 loopback 服务两种模式：只读工具发现与拒绝写入、真实预检、成果检查、原值读取、两次实际创建和交付包均通过。直接会话 EOF 后重开保持成果；loopback 创建尚未完成时断开 MCP，再连接后任务继续完成。证据在同一[验收记录](../prototype/qa/scientific-rgb-verification.json)的 `mcp` 字段。

实际生产 renderer 连接隔离原生服务，验证英文浅色 1440、中文深色 1024、英文深色 900 三种视口。界面实际提交并完成新的 MODIS 科学 RGB，名称保留在列表；成果地图在没有父文件时仍真实绘制，中心原值和交付包通过。三个视口的收起卡片均为 106 像素、无横向溢出，控制台和 CSP 无错误，浏览器没有外部请求。生成对话框沿用共享输入、主题、预检和按钮排列。截图已经人工检查；这仍不是原生桌面窗口验收。

HLS / VIIRS 的科学 RGB 路径已经接入，但没有测试账号，本记录不代表真实受保护原文件下载或生产三波段导出已验收。[软件内授权入口](provider-accounts.md)已经做好，后续在原生桌面内完成账号验证。完整产品与其他来源仍按[接入状态](provider-integration-status.md)继续推进。

## CLI、HTTP、桌面和 MCP

`scientific-rgb plan|run --request FILE` 接受 `{ "jobIds": ["red UUID", "green UUID", "blue UUID"], "projectId": "optional UUID", "name": "optional name" }`；可选 `qualityMask` 分别见[MODIS 请求](modis-rgb-quality-mask.md)和[Landsat 请求](landsat-rgb-quality-mask.md)，两种严格对象通过不同字段区分，不能混用。匹配的 Landsat 多景工程层使用相同请求，由预检生成[版本二同景来源清单](landsat-coupled-rgb.md)。`run` 等待任务完成；`inspect|package --id UUID` 和 `pixel --id UUID --x X --y Y` 使用已保存的成果。可通过 `--data-dir` 独立拥有目录或通过 `--server` 使用已有本地服务，不能同时打开桌面拥有的目录。

HTTP 为 POST `/rasters/rgb/plan`、POST `/rasters/rgb`、GET `/jobs/{id}/rgb` 和 GET `/jobs/{id}/rgb/pixel?x=&y=`。既有任务、文件、缩略图、来源 JSON 和交付包接口复用同一原生实现。Tauri 的四个命令分别为 `plan_scientific_rgb`、`run_scientific_rgb`、`inspect_scientific_rgb`、`sample_scientific_rgb`，受主窗口 ACL 约束。

MCP 提供只读 `geod_rgb_plan`、`geod_rgb_inspect`、`geod_rgb_pixel`；启动时显式启用 `--allow-write` 后才开放 `geod_rgb_run` 和 `geod_rgb_package`。创建仅接受原生来源任务 ID，不接受 URL、秘密或任意输出路径；按 `geod_job_status` 的终态和 `settled: true` 判定完成。见 [MCP 操作说明](mcp.md)。

可用下列可复现脚本验证已有的实际原文件；它们只使用 `.verification` 下的独立数据目录：

```sh
python scripts/verify-scientific-rgb.py --landsat-root .verification/local-rgb-native-20261002 --modis-root .verification/modis-native-20261002 --output .verification/scientific-rgb-new
python scripts/verify-scientific-rgb-mcp.py --root .verification/scientific-rgb-new
node scripts/verify-scientific-rgb-ui.mjs .verification/scientific-rgb-new
```

独立文件比对需要 Rasterio / NumPy。界面脚本使用当前生产 renderer、真实隔离原生服务和桌面 CSP，在自己的无界面 Edge 实例内运行；不操作用户桌面，也不代替原生 WebView 或安装包人工验收。本轮没有制作安装包或公开发布。
