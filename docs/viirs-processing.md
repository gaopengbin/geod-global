# VIIRS v002 本地波段准备与处理

2026-10-02。VNP09A1 / VJ109A1 / VJ209A1 v002 已接入 M5/red、M4/green、M3/blue 的本地准备、原始像元检查、RGB 显示、持久化缩略图与工程区域处理。**本页的正向文件验证全部使用独立生成的合成 HDF5，不代表 Earthdata 生产原文件已验收。**目录与公开浏览图的真实网络验收见[目录与授权记录](viirs-integration.md)。

## 工作流程

在设置中授权 Earthdata，下载完整 HDF5 原产品。在工程内分别准备 M5、M4、M3；任务会重新检查锁定的 HDF5 校验值、产品身份、全部三层原值和文件内网格，然后生成对应的原始单波段 GeoTIFF。重复准备复用已有任务。没有科学校验摘要的旧下载不能直接处理，可以重新下载一份已校验原产品；原文件保留。

准备后的文件可在工作空间中显示单波段，或由同一份原产品的三层组成本地 RGB。地图按实际正弦网格显示，像元检查返回原始 DN 和定标后的反射率。PNG 预览的灰阶或 RGB 拉伸只用于显示，不修改科学数值。缩略图沿用按原文件校验值绑定的原生磁盘缓存，重启后可复用。

工程可按波段裁剪区域，并使用现有相同坐标系、同间距网格的拼接流程。输出保持原始 Int16、NoData、定标和网格，不做重投影或重采样。此次 VIIRS 文件证据覆盖单景工程裁剪；相邻 VIIRS 生产瓦片、多时段重叠和带洞区域仍需补充专项验收，不能以其他产品的拼接测试代替。

## 原值、几何和来源

每层是 1200 × 1200 Int16，保留原填充值 -28672、比例 0.0001、偏移 0 和有效范围 -100..16000。范围之外的原值保留，不自动掩膜或钳制。依据为 [NASA VIIRS v2 用户指南](https://lpdaac.usgs.gov/documents/1657/VNP09_User_Guide_V2.pdf)。

几何来自 HDF-EOS 文件内的结构元数据。球体半径为 6371007.181 米，使用明确的 `VIIRS:Sinusoidal` 标识和 GeoTIFF 自定义投影参数，不虚构 EPSG 编号。此次夹具的实际间距约为 926.625 米；“标称 1 千米”和 MODIS 的 2400 × 2400 / 500 米参数不能替代这一网格。

每个准备任务固定原 HDF5 的任务 ID、SHA-256、完整科学摘要及原始数据集。编码后重新解码全部 Int16，逐值和规范小端 SHA-256 双重核对，成功后原子提交。取消、损坏原件、来源变更或元数据不一致都不会留下可用的成功结果。运行时不调用系统 GDAL、Python 或 HDF5 DLL。

准备文件和裁剪结果的来源清单保留原 HDF5 校验值、原始数据集、全部样本校验值、完整合成时段和 `qualityMaskApplied: false`。RGB 三层必须来自同一份锁定的原产品，不能混用不同下载或处理版本。原始 HDF5 保留全部字节；其他科学与 QA 层尚未解码，未应用质量掩膜。

## 当前验证

- 三个平台、九个准备 GeoTIFF，共 12,960,000 个 Int16，原生读回与独立 h5py/NumPy 的全样本哈希一致。原 HDF5 字节未改动。
- Rasterio 1.3.9 / GDAL 3.6.4 独立读取同一批九个文件，全部数值、投影参数、面积像元语义、NoData、比例与偏移一致。独立读取器只用于验证，不是软件运行时依赖。
- 原生任务链覆盖准备去重、DN 与反射率取值、本地 RGB、科学缩略图、单景工程裁剪、取消、来源变更、重启及中断重试。裁剪输出的 312 个 DN 与对应原始窗口逐值一致。
- 英文浅色 1440 和中文深色 1024 的无头浏览器检查了准备请求、三层缩略图、实际地图画布像素、窄窗口排布、脚本错误和横向溢出，并查看截图。UI 请求重放的是上述原生合成输出，不是一次 Earthdata 下载，也不代表正在运行的用户桌面已加载新代码。

[机器记录](../prototype/qa/viirs-processing-verification.json)区分文件验证、UI 重放与生产访问。[UI 夹具](../prototype/qa/viirs-prepared-fixture.json)明确标注合成来源，并移除本机文件路径。

回归通过 118 个 Node、118 个界面、179 个原生单元、3 个 CLI 和 4 个桌面边界测试；4 个需要外部条件的测试忽略。Clippy 全目标无警告，前端与开发桌面构建通过。

可用以下命令重现原生输出与 UI 夹具；生成物位于忽略目录：

```powershell
$env:GEOD_VIIRS_PREPARE_EVIDENCE = (Join-Path (Get-Location) '.verification/viirs-prepared')
cargo test --locked -p geod-runtime providers::viirs::prepare -- --nocapture
python -X utf8 scripts/export-viirs-preparation-fixture.py .verification/viirs-prepared/ui-fixture.json prototype/qa/viirs-prepared-fixture.json
python -X utf8 scripts/verify-viirs-preparation.py .verification/viirs-prepared
```

仍待验收：真实账号成功授权、三个平台生产 HDF5 的兼容性、相邻与重叠 VIIRS 原件的专项拼接，以及其他科学与 QA 数据层。公开浏览图、合成测试和其他产品的通过记录都不能替代这些证据。
