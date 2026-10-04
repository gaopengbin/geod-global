# MODIS 科学 RGB 质量筛选

2026-10-04。MOD09A1 / MYD09A1 v061 的科学 RGB 可选择按同景 QC 与 State 文件筛选像元。默认仍保留全部原始值；启用筛选后，合格像元的三个 Int16 DN 不变，被剔除的像元在三个通道中写为 `-28672` NoData。原文件不改变。

## 软件操作

1. 在工程中下载同景的 RGB 三个波段、波段质量和像元状态五份文件。
2. 打开本地 RGB，在图层工具中选择「生成科学 RGB」。
3. 选择「剔除云与云影标记」或「晴空且 RGB 波段质量最佳」，按需勾选「同时剔除雪冰」，填写成果名后提交。
4. 从工程或文件列表打开成果。地图说明、原始 DN 检查、文件来源和交付包保留筛选规则及质量文件的校验值。

同一景的区域裁剪、带孔多边形裁剪也可组合，但五份处理结果必须具有相同的来源、工程区域和像元网格。缺少匹配 QC / State 时，界面说明需要补充的文件，并保留不筛选的生成方式。切换规则会重新预检；已修改的成果名保留，预检未完成或规则不匹配时不能提交。

## 规则与数值

规则依据 [NASA MOD09 C61 用户指南](https://landweb.modaps.eosdis.nasa.gov/data/userguide/MOD09_User_Guide_V61.pdf)表 10、13。这是 GeoD 明确选用的标记筛选条件，不是大气精度保证。

| 选择 | 合格像元的条件 |
|---|---|
| 保留全部原始值 | 不应用质量规则；独立保留每个通道的 DN / NoData |
| `clear` | State 云状态位 0–1 必须为 0；云影位 2、卷云位 8–9、内部云位 10、邻云位 13 均为 0；两份质量文件均非填充值 |
| `clear_best` | 满足 `clear`；QC 的 MODLAND 位 0–1 和 RGB 对应波段 1、4、3 的质量代码均为 0 |
| `excludeSnow: true` | 额外要求 State 位 12 和 15 为 0。位 14 是盐滩，不能按雪冰剔除 |

云状态 3 的“默认晴空 / 未设置”不等于明确晴空，在这两种规则中剔除。QC 填充值为 `4294967295`，State 为 `65535`。合格负值、有效零和超过 1 的反射率仍保留；scale / offset、源正弦投影和网格保持不变，无重采样或重投影。

来源清单保存 `geod-modis-rgb-mask/v1`、规则、雪冰选项、两份质量文件的任务 ID、无签名 URL、完整 SHA-256 和处理来源。输出保存检查像元数、QA 剔除数、输入共同有效数及新剔除的共同有效数；前两个数可包含原先已经 NoData 的像元，不能混为新增损失。TIFF 元数据绑定完整规格，交付包保留规则、来源、计数与校验清单。

## 实际文件验收

使用此前由本软件下载的公开 COG，未新增上游请求。具体条目为 `MYD09A1.A2025177.h08v05.061.2025189031924`，合成时段 2025-06-26 至 2025-07-03。完整原件为 2400 × 2400，两个同景裁剪结果网格均为 56 × 20；带孔结果保留孔内 NoData。

三个输入范围各生成不筛选、`clear`、`clear_best`、`clear_best + excludeSnow`，共十二份成果。独立 Rasterio / GDAL 核对 **69,146,880 个通道值**和 **2,368,256 个预览 RGBA 像素**，全部一致；CRS、仿射变换、标定、NoData、各通道摘要、原值取样和十二份 ZIP 成员 / CRC 均通过。

| 完整原件 | 输入共同有效像元 | 新剔除的共同有效像元 |
|---|---:|---:|
| `clear` | 5,759,997 | 1,088,853 |
| `clear_best` | 5,759,997 | 1,578,419 |
| `clear_best + excludeSnow` | 5,759,997 | 1,578,419 |

此瓦片的雪冰选项没有再剔除有效像元，两个小范围裁剪的新增剔除数也为零；不把它们宣称为真实雪冰剔除正样本。云位、RGB 质量位、非 RGB 位、QC 最高位、雪冰位与盐滩位、填充值及取消另以明确标注的合成文件回归验证。

实际 stdio MCP 的直接目录与本地服务两种模式都完成创建、原值读取和交付包；只读模式拒绝写入，错误规则与重复质量 ID 拒绝，断开后重新连接保留已提交任务。重启后规格和结果一致。另在没有任何父级 RGB / QA 文件或任务的隔离副本中，十二份成果的检查、原值、交付包和缩略图通过；十二个磁盘缓存条目在重启后字节、文件身份和创建时间不变，全部 111,360 个缩略图 RGBA 像素独立一致。修改成果、损坏缓存、改变规则清单的控制也通过。

界面验收覆盖 1440 像素英文亮色、1024 像素中文暗色和 900 像素英文暗色三个视口，分别打开完整原件、矩形裁剪和带孔裁剪的成果。通过实际按钮创建质量筛选 RGB，核对修改后的名称、规则、质量文件 ID、输出摘要和原值；地图实际绘制的源预览 RGBA 与原生预览全部一致，没有横向溢出、脚本错误、CSP 违规或上游请求。收紧生成对话框的间距，科学 RGB 图层只保留一次类型说明；原值面板展开时隐藏重复的底部地图说明。

机器证据见[验收记录](../prototype/qa/modis-rgb-mask-verification.json)。软件界面验证使用当前构建资源、精确桌面 CSP 和真实原生接口，在隐藏 Edge 中运行；源预览一致不代表逐像素比较了缩放后的地图画布，也不代替原生 WebView / 安装版人工验收。

## 调用与边界

CLI、HTTP、Tauri 和 MCP 沿用[科学 RGB](scientific-rgb.md)的统一请求，增加可选字段：

```json
{
  "jobIds": ["red UUID", "green UUID", "blue UUID"],
  "qualityMask": {
    "qcJobId": "QC UUID",
    "stateJobId": "State UUID",
    "policy": "clear_best",
    "excludeSnow": true
  },
  "name": "晴空 RGB"
}
```

本页和对应机器记录保留同景 v1 的首次验收边界：当时独立多景 RGB / QA 拼接不能证明每个像元选择了同一景，预检拒绝筛选。后续新增的[多景联合质量选择](modis-coupled-rgb.md)重新读取全部原件，在选景时联合检查五份文件，不直接对独立拼接值套用掩膜。其他 MODIS 产品、Landsat / HLS / VIIRS 的质量规则和通用网格变换仍待实现。

下列命令对应原同景 v1 验收二进制（`990f75c3bcfac3ca`），其中包含旧版拒绝多景的控制；当前多景实现的验收命令见[多景说明](modis-coupled-rgb.md)。命令仅写入新建的 `.verification/modis-rgb-mask-*` 目录，保留原验收文件；同一目录不可同时由直接 MCP 和服务拥有：

```text
python -X utf8 scripts/verify-modis-rgb-mask.py --root .verification/modis-rgb-mask-new --exe .verification/modis-rgb-mask-final-20261004/runtime-990f75c3bcfac3ca.exe --port 4621
python -X utf8 scripts/verify-modis-rgb-mask-mcp.py --root .verification/modis-rgb-mask-new --port 4623
node scripts/verify-modis-rgb-mask-ui.mjs .verification/modis-rgb-mask-new
python -X utf8 scripts/verify-modis-rgb-mask-cache.py --source .verification/modis-rgb-mask-new --root .verification/modis-rgb-mask-offline-new --port 4627
python -X utf8 scripts/summarize-modis-rgb-mask.py --qa .verification/modis-rgb-mask-new --offline .verification/modis-rgb-mask-offline-new
```

原生运行不依赖 Python / GDAL；它们只用于独立验收。此次未制作安装包、远程发布或宣称全部数据源接入完成。原先通过的 renderer 已保存完整资源快照，新的规则按当前资源与原生二进制另立验收记录。
