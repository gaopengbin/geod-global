# Landsat 科学 RGB 质量筛选

2026-10-04。Landsat 8/9 Collection 2 Level-2 的科学 RGB 生成对话框新增可选质量筛选。默认仍为「保留全部原始值」；用户明确选择规则后，应用读取同景的 QA_PIXEL / QA_RADSAT，保留合格的三通道 DN，将剔除像元的三个通道一同设为 NoData。

## 软件操作

1. 下载同一景 B4 / B3 / B2、QA_PIXEL 和 QA_RADSAT。探索页可选「RGB 波段 + 像元质量 + 饱和度 · 5 个文件」，旧工程也可补下同一处理版本的两份质量文件。
2. 在工作空间打开对应的本地 RGB，点击组合图层的「生成科学 RGB」图标。
3. 选择保留全部值、常规无云标志或更保守的无云标志，可另选剔除雪冰。缺少匹配质量文件时显示原因，不能提交筛选请求。
4. 完成预检后提交。在所属工程或任务页查看进度；成果卡片可打开地图、检查三通道原值、查看质量规则与来源校验值，并生成交付 ZIP。

同一景的工程矩形裁剪或带孔多边形也可筛选：先为五类文件分别生成同一区域的成果，再打开对应 RGB。五份文件必须来自同一原始处理目录，工程、区域、来源顺序和像元网格均须匹配；名称相同或尺寸相同不能代替校验。多景工程使用[联合质量选景](landsat-coupled-rgb.md)，重新读取每景五份原件，不直接套用各自拼好的质量值。本页保留首次 Landsat 9 同景验收的范围和证据。

## 筛选语义

位字段依据 [USGS Collection 2 Quality Assessment Bands](https://www.usgs.gov/landsat-missions/landsat-collection-2-quality-assessment-bands)。以下两种选择是本软件公开定义的规则，不是新的 USGS 产品或精度保证。

| 选项 | 接受条件 |
|---|---|
| 保留全部原始值 | 不读取质量层，沿用原始三通道科学 RGB 规则 |
| 常规无云标志 `cloud_free` | QA_PIXEL 填充、膨胀云、高置信卷云 / 云 / 云影位均未设置；QA_RADSAT 的 RGB 波段 B2 / B3 / B4 饱和及地形遮挡位均未设置 |
| 更保守的无云标志 `cloud_free_conservative` | 满足上一行，并要求 Clear 位设置、云 / 云影 / 卷云置信度明确为低；未设定置信度、较高或保留编码不当作低置信；未使用的饱和层位必须为零 |
| 可选剔除雪冰 | 两种规则均拒绝高置信雪位；更保守规则另要求雪冰置信度明确为低 |

水体标志不导致剔除。非 RGB 波段饱和不导致剔除，包括 QA_RADSAT 的 B9 位；它与未使用位不同。质量层保留 UInt16 原始位值，工程内部覆盖掩膜另行检查；QA_RADSAT 的有效零值不当成未覆盖区域。

接受像元不截断、不缩放、不平滑；三通道仍为 UInt16、NoData 为 0、scale 为 0.0000275、offset 为 −0.2。输出保留原始样本中心与 Point / Area 采样解释，没有重采样或重投影。预览拉伸只用于显示，PNG 不代替科学文件。

来源清单固定三份 RGB 和两份质量文件的 SHA-256、大小、无签名源路径和处理参数。成果保存选择规则、雪冰选项及四个计数：检查像元、剔除像元、筛选前共同有效像元、损失的共同有效像元。成功成果可在父级文件移走后独立读取、恢复缩略图和导出；重新生成仍需要原始来源通过校验。

## CLI、HTTP、桌面与 MCP

既有 `scientific-rgb plan|run`、HTTP `/rasters/rgb/plan` / `/rasters/rgb`、桌面命令和 `geod_rgb_plan` / `geod_rgb_run` 共用以下可选字段：

```json
{
  "jobIds": ["red task UUID", "green task UUID", "blue task UUID"],
  "qualityMask": {
    "qaPixelJobId": "QA_PIXEL task UUID",
    "qaRadsatJobId": "QA_RADSAT task UUID",
    "policy": "cloud_free_conservative",
    "excludeSnow": false
  }
}
```

任务 ID 须为本地已完成来源；重复 ID、错景、不同处理版本、不同网格或 MODIS 规则拒绝排队。MCP 只读模式仍拒绝生成与交付，写入需显式启动 `--allow-write`。完成后按 `settled: true` 和成功终态确认成果。

Landsat 清单使用 `geod-landsat-rgb-mask/v1`，源顺序固定为 `qa_pixel` / `qa_radsat`。既有 MODIS 的 `qcJobId` / `stateJobId` 请求与 `geod-modis-rgb-mask/v1` 清单保持兼容，两种产品不混用规则。

## 实际文件验证

真实文件验收使用独立目录，复用此前完整原生下载的 Landsat 9 `LC09_L2SP_044034_20250628_02_T1` 五份文件，处理目录为 `LC09_L2SP_044034_20250628_20250629_02_T1`。本次没有新增供应商下载；原文件与此前已验收的运行程序保留。

完整整景、同景矩形和带孔区域分别核对默认不筛选、常规筛选、更保守筛选及更保守筛选加雪冰选项。独立 Rasterio / GDAL 对照每个 DN、完整网格、标定参数、有效数、质量统计、显示 RGBA、原值与 ZIP 来源清单；真实生产 renderer、直接 / 本地服务 MCP 和没有父级文件的持久缓存分别验收。

十二份实际成果的 **878,416,812 个通道值**、4,749,312 个预览 RGBA 像素和 126 个原值查询全部独立一致。三类区域的每个原始、未筛选成果分别作为对照；统计中的「损失」只计入筛选前共同有效的 RGB，不将原始填充区域算作新增损失。

| 范围 | 网格 | 筛选前共同有效 | 常规损失 | 严格损失 | 严格并剔除雪冰损失 |
|---|---:|---:|---:|---:|---:|
| 完整整景 | 7671 × 7791 | 40,715,194 | 14,276,837 | 14,279,186 | 14,279,186 |
| 同景矩形 | 3612 × 1860 | 4,918,437 | 2,526,496 | 2,527,335 | 2,527,335 |
| 同景带孔区域 | 3612 × 1860 | 4,656,199 | 2,305,583 | 2,306,422 | 2,306,422 |

十份单波段工程输入另与原始完整文件和多边形独立核对，共 67,183,200 个 DN、26,873,280 个质量覆盖位。带孔范围排除 276,690 个网格像元；有效饱和层零值和原始水体 / 非 RGB 饱和像元保持相应语义。十二个 ZIP 的 CRC、全部五个成员、原始 TIFF、规则清单和校验值通过，重启后规则与结果不变；错景、重复质量任务、拿 RGB 冒充质量层及混用 MODIS 规则均拒绝。

[完整机器记录](../prototype/qa/landsat-rgb-mask-verification.json)绑定原生、MCP、生产界面和离线缓存证据。直接 / 本地服务两种 MCP 创建两份同景工程 RGB，40,309,920 个 DN 与六个原值点独立一致；只读拒绝生成、工具模式、协议 EOF、断连后继续完成及重新连接均通过。

生产 renderer 在 1440 英文浅色、1024 中文深色和 900 英文深色三个视口真实生成 / 打开成果，核对实际绘制的源 RGBA、原值、显示的剔除计数及两份质量来源哈希，没有外部请求、页面 / CSP 异常或横向溢出；截图已实际查看。十二份成果移走全部父级任务后仍可离线读取、取值及交付，十二张缩略图的全部 205,440 个 RGBA 像素独立一致；重启保留缓存内容、文件身份和创建时间。

完整运行时回归 473 项通过、4 项明确忽略，严格 Clippy、格式和相关前端逻辑 / 界面回归通过。质量位组合、UInt16 极值、Point / Area、不同来源、网格不符、取消、篡改输出及移除父级后读取另有合成回归；它们不能代替真实正例。

验收运行时 SHA-256 为 `51a09564e71c921f6f5ddb11e3b3cad8b19a2ed8ece139eda74e7d8642756774`。带本地资源的开发程序为 `desktop-ec2018887323f151.exe`，80,918,528 字节，SHA-256 为 `ec2018887323f15127da1bb35d91a530601dee9697dd84a6789ea52069fe496f`；完整 662 个前端文件冻结。此前 MODIS、Landsat 原始质量和质量工程程序 / 资源保留，原开发运行程序未替换。

## 可复验步骤与边界

以下脚本使用 `.verification` 下的新隔离目录和已下载原件；不操作用户桌面、不制作安装包，也不发布软件：

```sh
rtk proxy python -X utf8 scripts/verify-landsat-rgb-mask.py --root .verification/landsat-rgb-mask-new --exe .verification/naip-native-target/debug/geod-runtime.exe
rtk proxy python -X utf8 scripts/verify-landsat-rgb-mask-mcp.py --root .verification/landsat-rgb-mask-new
rtk proxy node scripts/verify-landsat-rgb-mask-ui.mjs .verification/landsat-rgb-mask-new
rtk proxy python -X utf8 scripts/verify-landsat-rgb-mask-cache.py --source .verification/landsat-rgb-mask-new --root .verification/landsat-rgb-mask-cache-new
```

真实 Landsat 8 RGB 和多景联合质量选景的最新进度见[专项说明](landsat-coupled-rgb.md)，不扩写本页历史记录的验收范围。其他质量层、气溶胶质量、热红外产品、通用重投影及更大成果不包含在本项中。质量标志选择不能证明大气校正精度。合成极值、雪冰 / 遮挡 / 保留编码等控制单独标为回归测试，不冒充真实遥感正例。

汇总入口另通过四项控制：失败界面记录、不同运行程序、错误原生记录引用和过期资源均拒绝登记。控制只修改内存中的记录副本或独立资源副本，完整通过记录和历史程序保持原样。

界面验收使用生产资源、实际桌面 CSP、原生接口桥接与隐藏浏览器，不能替代原生 WebView 窗口验收。NASA / Copernicus 的[软件内授权入口](provider-accounts.md)已接入；没有账号时，成功授权和受保护原文件仍保持待验收。
