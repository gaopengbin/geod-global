# Landsat 多景科学 RGB 与质量选择

2026-10-04。Landsat 8/9 Collection 2 Level-2 的科学 RGB 可从匹配的多景工程层重新读取原件，按质量选择同一景的完整三通道。默认仍保留全部原始值；明确选择质量规则才使用此流程。真实原件、原生处理、界面、MCP 和离线恢复的互相绑定证据见[完整验收记录](../prototype/qa/landsat-coupled-verification.json)。

## 软件操作

1. 工程内每景下载 B4 / B3 / B2、QA_PIXEL 和 QA_RADSAT 五类原文件。
2. 为五类文件生成同一工程区域的图层，区域可以是矩形或带孔多边形。场景集合、顺序、处理版本、区域和网格必须匹配。
3. 在工作空间打开对应工程 RGB，点击「生成科学 RGB」，选择常规或严格晴空规则，可另选剔除雪冰。
4. 预检显示输出尺寸与完整临时空间预算，提交后在后台处理。成果可打开地图、读取三通道 DN / 反射率、查看每景贡献及回退像元数，或生成科学 RGB ZIP。

工程图层固定输出范围与来源；质量选择重新读取每景的五份完整原件。缺少原件时提示原因，不能用各自拼好的质量值替代同景质量。已完成的成果、缩略图、原值和交付包可独立读取，重新生成仍需要原件。

## 同景选择与网格

每个输出像元先检查同一景的三份 RGB 是否均有效，再读取这一景的 QA_PIXEL / QA_RADSAT。合格候选按采集日期从早到晚覆盖，相同日期按条目 ID 决定顺序。较新景有云、质量不符或缺少任一通道时，保留较早的合格完整三通道；没有合格候选时三个通道一起写为 NoData。

规则沿用[同景质量筛选](landsat-rgb-quality-mask.md)，位字段依据 [USGS 原始定义](https://www.usgs.gov/landsat-missions/landsat-collection-2-quality-assessment-bands)。水体与非 RGB 饱和标志本身不剔除；严格规则要求明确低置信度，保留编码不视为低置信度。显示拉伸不写入 DN，质量选择不证明大气校正精度。

输入须为相同 UTM CRS、30 米整数对齐网格；不同场景的外边界和尺寸可以不同。原始 Point 样本中心保留，工程输出按 Area 范围记录；写入的 GeoTIFF 栅格解释与坐标标签同步。三通道仍为 UInt16、NoData 0、scale 0.0000275、offset −0.2，不进行重采样或重投影。

## 保存规则与接口

CLI、HTTP、桌面和 MCP 继续使用[现有请求](landsat-rgb-quality-mask.md)。单景清单保持 `geod-landsat-rgb-mask/v1`，多景质量清单使用 `geod-landsat-rgb-mask/v2`：

- `coupled.scenes` 保存各景五份原件的任务 ID、无签名来源、大小、SHA-256 和原始网格。
- `coupled.selection` 固定采集日期与条目 ID 的选择顺序；`geometry` 保存工程区域。
- 输出的 `coupled.sceneValidPixels` 记录每景最终贡献；`fallbackPixels` 记录较早合格景替换较新完整候选的像元数。
- 「无合格 RGB 的像元」包含原始无数据和区域外 / 孔洞；「剔除的原有效像元」只计筛选前原本有完整 RGB 的损失。

匹配工程层与每景五份原件绑定，不能混用不同区域、来源顺序或处理目录。MCP 只读模式继续拒绝生成；已保存的成果与规则不依赖父级任务仍在目录内。

## 实际文件验收

原生完整文件矩阵和重启检查已通过。使用三景实际公开文件：`LC09_L2SP_044034_20250612_02_T1`、`LC08_L2SP_045034_20250627_02_T1`、`LC09_L2SP_044034_20250628_02_T1`。新增六份完整 RGB 原生下载，共 571,534,841 字节；九份此前已验收原件复用，合计十五份原件，原文件和校验值保持。

完整 Landsat 8 验证默认不筛选、常规、严格及严格加雪冰选项四份成果；两景云区矩形 / 带孔范围、三景矩形 / 带孔范围分别验证三种质量选择，共十六份成果。独立 Rasterio / GDAL 核对全部 **871,015,284 个 DN**、3,919,272 个预览 RGBA 像素和 44 个原值点，同时核对全部网格、标定参数、完整三通道和 ZIP 五个成员 / CRC / 校验值。

| 范围 | 网格 | 原始共同有效像元 | 常规保留 | 严格保留 | 常规 / 严格回退 |
|---|---:|---:|---:|---:|---:|
| Landsat 8 完整原件 | 7751 × 7881 | 40,597,830 | 4,328,212 | 4,300,702 | 不适用 |
| 两景云区矩形 | 42 × 22 | 924 | 915 | 914 | 915 / 914 |
| 两景云区带孔范围 | 42 × 22 | 768 | 759 | 758 | 759 / 758 |
| 三景矩形 | 4121 × 1860 | 7,665,058 | 3,505,486 | 3,497,498 | 851,095 / 850,836 |
| 三景带孔范围 | 4121 × 1860 | 7,288,313 | 3,341,649 | 3,333,720 | 734,147 / 733,888 |

真实样本的雪冰选项没有新增变化，雪冰、饱和、缺通道及无符号极值的控制单独标为回归测试。孔洞与区域外按像元中心排除；两种带孔网格分别排除 156 / 376,745 个像元。回退数不包含较早景在其他景之外的正常边缘覆盖。错误规则、不同区域 QA、重复质量任务及缺少原始 QA 均拒绝排队，十六份成果重启后可检查，十五份源文件不变。

直接目录与本地服务两种 MCP 各创建一份实际 RGB，独立核对全部 5,544 个 DN 和六个原值点。只读模式拒绝生成，错误策略与重复质量任务被拒绝；本地服务在适配器断开后继续完成，重新连接可读取与交付，协议正常结束。

十六份成果只保留完整 TIFF 和清单，移除全部三十五个父级任务 / 文件，在不可达上游代理下仍可检查、取值和生成科学 RGB ZIP。十六张缩略图的全部 175,144 个 RGBA 像素独立一致；重启保持缓存字节、文件身份和创建时间。改动成果拒绝命中旧缓存，损坏缓存 JSON 后可正确重建，篡改保存规则拒绝交付。

生产界面的 1440 像素英文浅色、1024 像素中文深色和 900 像素英文深色三组检查通过；实际按钮创建一份多景质量 RGB，地图实际绘制的全部 267,576 个源 RGBA 像素、原值、质量损失数、每景五份原件链接 / 校验值和缩略图分别核对。地图读取占用线程时，预检实际收到一次忙碌响应，自动等待后按最新雪冰选项完成，保留用户修改的成果名。切换规则会取消旧等待，关闭窗口会停止重试；持续忙碌有一分钟上限，其他文件错误立即显示，生成操作不自动重复提交。相关科学 RGB 界面回归共十七项通过。

十一个验收汇总反例拒绝错误或不完整的证据，未修改已接受记录。当前桌面开发程序为 80,929,280 字节，SHA-256 为 `247208aceed24ec48d77ae3a29386e814104710b48dd799c22384fd375bb8b3c`；完整 662 个生产资源和程序按哈希冻结，详情在验收记录的 `rendererSnapshot`。原 Landsat 同景验收快照和原开发运行时保持不变。本轮没有制作安装包或发布。

## 复验与范围

脚本使用新的隔离目录和实际下载原件，保留此前已验收文件：

```sh
rtk proxy node scripts/verify-landsat-coupled-sources.mjs .verification/landsat-coupled-sources-new
rtk proxy python -X utf8 scripts/verify-landsat-coupled.py --source .verification/landsat-coupled-sources-new --root .verification/landsat-coupled-new --exe .verification/naip-native-target/debug/geod-runtime.exe
rtk proxy python -X utf8 scripts/verify-landsat-coupled-mcp.py --root .verification/landsat-coupled-new
rtk proxy node scripts/verify-landsat-coupled-ui.mjs .verification/landsat-coupled-new
rtk proxy python -X utf8 scripts/verify-landsat-coupled-cache.py --source .verification/landsat-coupled-new --root .verification/landsat-coupled-offline-new
rtk proxy python -X utf8 scripts/verify-landsat-coupled-summary.py --root .verification/landsat-coupled-new --offline .verification/landsat-coupled-offline-new
rtk proxy python -X utf8 scripts/freeze-accepted-renderer.py --ui .verification/landsat-coupled-new/ui-verification.json --root .verification/renderer-landsat-coupled-new
rtk proxy python -X utf8 scripts/summarize-landsat-coupled.py --source .verification/landsat-coupled-sources-new --qa .verification/landsat-coupled-new --offline .verification/landsat-coupled-offline-new --snapshot .verification/renderer-landsat-coupled-new
```

其他质量层、气溶胶、热红外、通用重投影、更大文件及通用工程成果 ZIP 仍另行接入。实际桌面 CSP 和原生接口桥接下的隐藏界面验收不替代原生 WebView 窗口验收。NASA / Copernicus 的[软件内授权入口](provider-accounts.md)已接入，没有账号时成功授权与受保护原文件验收仍保留待完成。
