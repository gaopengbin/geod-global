# GeoD Global — 可执行处理、配方与 CLI

日期：2026-09-22。此轮将现有真实检索、下载和SCL预览继续接到本地裁剪、持久化配方与命令行。完整产品范围仍由01–06约束；本文件记录具体已实现的子集，不能将它扩展解释为通用GIS处理器。

## 可执行数据流

```text
Earth Search真实目录 → 原始SCL下载 → SHA-256固定源文件
    → 工作区范围或手工边界 → 处理预检 → 持久化裁剪任务
    → GeoTIFF完整回读验证 → 来源清单 → 我的数据中检查成果
                           ↘ 配方保存 / JSON导入导出 / CLI重跑
```

源坐标裁剪使用栅格的WGS84 UTM米坐标。EPSG:4326输入使用经纬度，四条边各取64段投影采样，得到源坐标系中的矩形外包框，再与源范围求交，并向外对齐完整像元。输出保留源CRS、像元大小、UInt8分类值和NoData，不对分类值插值。界面和记录同时保留请求范围及实际像元窗口。

经纬度输入不支持跨越日期变更线或UTM纬度范围之外的区域。输出是普通GeoTIFF；COG结构、重投影、曲线边界掩膜、多波段、拼接及指数运算尚未实现。WGS84范围的投影外包矩形也不等于精确多边形裁剪。

## 配方与成果契约

可执行配方版本为 `geod-raster-recipe/v1`，使用严格数据对象：名称、源任务ID与SHA-256、clip操作的坐标系/范围、GeoTIFF输出格式。它与原型的 `design-prototype/v1` 以及规格中的通用Core契约草案不同。未知版本、操作、字段或格式会被拒绝，不能导入任意命令、文件路径或新的远程来源。

正式结构见[当前可执行配方Schema](../schemas/raster-recipe-v1.schema.json)。示例[源资产下载请求](../examples/sentinel-scl-download.json)与[真实验收配方](../examples/sentinel-scl-clip.recipe.json)有不同用途：后者固定本机验收源任务；在其他存储目录使用时，须先获得同一源文件，再填写该目录中的任务ID与实际SHA-256。导入不会自动下载、保存或执行。

真实配方保存在运行时目录的 `recipes.json`，与任务记录共用进程目录锁。保存会执行真实预检；运行时再次检查源文件，不能只信任先前的预检或记录中的旧哈希。来源缺失或改变时不能生成成功成果。前端编辑参数后会使旧预检失效。

裁剪任务沿用queued/running/succeeded/failed/cancelled/interrupted状态，`kind=raster_clip`，记录parentId、完整recipe和实际crop plan。旧下载记录缺少kind时仍按download读取。重启不会自动执行中断的处理任务，重试显式从头开始。

每个成果包含生成的UUID命名GeoTIFF与同名 `.metadata.json` 来源清单。清单为 `geod-raster-artifact/v1`，保留相对成果文件名、字节数、哈希、原始数据项/URL/署名/源哈希、配方和实际裁剪信息。两份文件及任务记录提交后才显示成功；不覆盖原始文件或其他成功成果。

## 各入口共用核心

桌面Tauri命令、loopback API和CLI都调用同一个JobManager及Rust处理器，不依赖Python/GDAL，也不引用国内版checkout。Rasterio/GDAL仅作为开发QA的独立参照。

API增加 `GET /recipes` 与 `POST /recipes/plan`、`POST /recipes`、`POST /recipes/run`。POST仍受来源、Host、客户端标头和请求体限制。plan不创建任务或成果；run生成真实异步任务。

CLI提供配方预检、保存、运行，以及下载和任务查询；会等待启动的执行任务进入终态并完成worker清理才退出，stdout用于JSON结果，失败返回非零退出码。与已运行的浏览器服务协同时使用loopback模式，直接存储模式仍遵守独占目录锁。完整命令和源任务重绑定步骤见[英文操作教程](../docs/workflows/clip-sentinel-scl.md)。

```powershell
cargo build --locked -p geod-runtime
.\target\debug\geod-runtime.exe recipes plan --recipe examples/sentinel-scl-clip.recipe.json --server http://127.0.0.1:4318
.\target\debug\geod-runtime.exe recipes run --recipe examples/sentinel-scl-clip.recipe.json --server http://127.0.0.1:4318
```

上例固定本次验收目录的源任务ID。使用空目录时按教程先下载，再把源ID和SHA-256写入配方，不能直接假设示例ID跨设备存在。

## 验收

### 真实像元与来源

使用已有原始SCL `S2C_10SEG_20250707_0_L2A`，源SHA-256为 `ede35bce788bbafd2c0dbda4bca8c0b56c30fbf1027b37db63d8c5ee92e8b1d8`。在中文界面选择工作区范围 `[-122.55,37.68,-122.32,37.84]`，完成预检、保存及执行：

- 成果任务 `22db8368-9ecf-4076-9b33-7af7814fa19d`，1020×895像元、单波段UInt8、20米、EPSG:32610、NoData=0。
- 独立像元窗口 `[1980,585,1020,895]`，实际米坐标边界 `[539580,4170400,559980,4188300]`。
- GeoTIFF为46,315字节，SHA-256 `af78deb054871d1f6b5c02de84f40fddbeac060ecd5d79781c305a7aa4063ecd`。
- Rasterio/GDAL独立投影求窗、读取全部912,900个像元，检查CRS、变换、像元大小及NoData，全部相同；源文件校验值不变，sidecar与任务记录一致。
- 原生派生预览为768×673；516,864个预览像素与最近邻参照逐像素相同，全分辨率分类统计一致。
- 英文界面重跑同一配方得到新任务 `4c99bc6d-2032-4997-b948-5409a90ba7ef`，TIFF大小和SHA-256完全相同。

机器证据：[裁剪对照](../prototype/qa/crop-wgs84-verification.json)、[预览对照](../prototype/qa/crop-preview-verification.json)、[重复运行对照](../prototype/qa/crop-rerun-verification.json)。实际TIFF与JSON位于忽略目录 `.geod-global/assets/`；它们不是仓库内置的伪造处理样本。

另以源坐标 `[520001,4110001,580009,4180009]` 验收非对齐边界，独立得到窗口 `[1001,999,3001,3501]`；全部10,506,501个输出像元和空间标签一致，见[源坐标对照](../prototype/qa/crop-source-crs-verification.json)。

### 界面与交互

已验证中文保存并执行、成果检查、配方JSON审阅、未知版本拒绝、合法配方导入后显式预检、修改输入使Save/Run失效、英文界面重跑。自定义中文名称在语言切换时原样保留。1440×1000及390×844截图验收通过，窄屏无横向页面溢出，弹窗内容滚动且操作按钮保持可用。截图见 `prototype/qa/crop-*.png`。

JSON导出内容与保存配方一致；内嵌浏览器的文件保存事件仍未证明落盘，提供完整文本审阅/复制入口。CLI的配方文件与实际TIFF/sidecar落盘另有验证。桌面进程启动检查不能替代原生GUI逐项操作验收。

### CLI、重启与构建

- 空目录真实CLI依次执行download → plan → save → run → list/status/inspect → rerun，均输出可解析JSON并以0退出。重开的进程读取到配方、成果与来源清单；重跑TIFF哈希与浏览器一致。见[独立CLI闭环](../prototype/qa/cli-workflow-verification.json)。
- 重启4318服务后，已存配方和成果仍可用；`--server`成功运行，等待终态且settled，错误源哈希被拒绝且不新增任务。见[服务适配验收](../prototype/qa/cli-server-verification.json)。
- 真实回读发现并修复serde_json默认解析造成的一位浮点漂移，启用float_roundtrip并添加回归测试。取消后的CLI也必须等worker清理完成才返回，增加终态但未settled的回归。
- `npm run verify`通过：仓库隔离、7张样本校验、文档链接、草案及实际配方Schema、25项JS测试、生产资源构建。`cargo test --locked --workspace`通过40项Rust测试（34核心、3 CLI、3桌面边界）；runtime严格Clippy通过。
- `cargo build --locked -p geod-global-desktop --features custom-protocol`成功。新的Windows调试EXE为22,968,320字节，SHA-256 `73ed533fd6dfd3e225e93603be88ba5ee4def9a4327c832f104090a182a07d59`；启动3秒仍响应、stderr为空。完整构建指纹见[构建记录](../prototype/qa/processing-build-verification.json)。尚未生成签名安装包，也未完成原生GUI全流程和跨平台发行验收。

## 完整工作包的当前覆盖

| 工作包 | 当前覆盖 | 仍需推进 |
|---|---|---|
| D01–D04 | 已认可工作区、中英文、多页面与本轮状态 | 更大数据量、高DPI、其他语言与全部专业场景 |
| A01–A02 | 独立仓库、桌面ID、内部Job/Recipe/Crop契约 | 通用数据模型迁移与稳定API发布 |
| P01–P05 | Earth Search Sentinel-2查询与白名单源下载 | 多Provider、认证、自定义/商业来源、地图服务 |
| J01–J02 | 下载与处理持久化、取消、重试、原子成果提交 | 字节续传、全面磁盘故障注入与长期恢复 |
| V01–V03 | 同网格缩略图比较、原生SCL像元预览 | 通用二维地图、多波段拉伸、3D与坐标联动 |
| X01 / F01–F03 | 本轮SCL矩形裁剪、回读验证、成果与来源清单 | 通用栅格、重投影、掩膜、COG、成果文件管理 |
| W01–W03 | 固定本地源配方及CLI闭环 | 批量区域、版本迁移、跨设备输入绑定与MCP适配 |
| C / PAY / O | 完整规格和边界保留 | 选定的云功能、商业准入、真实收款及运营验证 |
| G / Q | 真实工作流教程与本地构建验收 | 公开官网、用户研究、跨平台安装/签名/更新和推广 |

每行是工作包的部分覆盖，不将单个SCL流程标为整个工作包verified或released。云服务、商业化及市场工作不会因本地工程先行而从完整目标中删除。
