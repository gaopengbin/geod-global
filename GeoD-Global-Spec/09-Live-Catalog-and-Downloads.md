# GeoD Global — 实时目录与真实下载验收

日期：2026-09-22。完整产品范围继续沿用01–06，本文件只记录当前已实现的链路和本轮验收结果。

## 实现范围

独立仓库新增 `crates/geod-runtime` 和 `src-tauri`。桌面命令和浏览器的loopback调试服务复用同一个Rust JobManager；没有引用、迁移或修改国内版源码。桌面应用ID为 `xyz.laogao.geod.global`，配置与下载记录独立。

前端保留已认可的视觉方向，并提供明确的Sample catalog / Live catalog选择。Live模式通过Earth Search检索Sentinel-2 L2A，可编辑WGS84范围、UTC日期、云量和分页大小。支持30秒超时、取消、过期请求隔离、错误、空结果和继续分页。实时缩略图按来源显示，不绘制未经验证的区域轮廓；对比要求相同CRS、transform和shape。

Download source asset下载完整SCL GeoTIFF、真彩色GeoTIFF或JPEG缩略图。界面明确说明整景下载、文件上限及验证范围；Tasks和My Data展示实际字节、文件路径、SHA-256与源URL。设计任务和报告继续保留在独立折叠区。

Rust核心支持两个并发下载、最多64个活动/排队任务、每文件512 MiB上限、目录互斥锁、任务JSON持久化、取消及从头重试。启动时将未完成任务标为interrupted，不能假定已成功或自动恢复传输。只接收指定Sentinel COG存储桶的无签名HTTPS资产，不跟随重定向，也不允许请求指定任意本地输出路径。

## 实际浏览器与文件验收

使用浏览器页面 `http://127.0.0.1:4317`，Rust服务监听 `127.0.0.1:4318`，数据目录 `.geod-global/`。这些文件被Git忽略。

| 检查 | 本轮观察 |
|---|---|
| 实时目录 | 范围 `[-122.55,37.68,-122.32,37.84]`，UTC 2025-07-01至2025-07-07、云量≤60%；返回2条七月场景，区别于本地六月样本 |
| 参数一致 | 页面日期框与实际Showing查询日期一致；修复原日期控件事件导致旧值提交的问题，submit读取当前FormData |
| 非法范围 | `181,0,182,1`被拦截，显示可操作说明；旧结果仍带原查询范围，未冒充新查询 |
| 空结果 | 同区域/日期、云量0%返回空结果；Reset filters放宽云量到100%并重新请求，恢复2条结果 |
| 分页 | 另以官方API实测每页2条，沿next取第二页，记录不重复；修复Earth Search所需 `sortby=-properties.datetime` |
| 下载 | 页面选中 `S2C_10SEG_20250707_0_L2A` 的 `scl`，创建真实任务 |
| 取消/重试 | 下载约1.6 MiB时在页面取消，显示cancelled和无完成产物；Retry from start从0开始，attempts变为2并成功 |
| 成果库 | My Data展示真实路径、2.3 MiB、SHA-256及源资产URL；设计报告单独折叠 |
| 持久化 | 停止并重启Rust服务，成功记录及校验值保持不变；异常退出运行任务恢复另由Rust确定性测试覆盖 |
| 响应式 | 1440×900工作区及390×844搜索/成果库截图已查看；窄屏表单和列表可滚动，未出现页面横向溢出 |
| 图片可读性 | 给影像文字增加深色底板，避免高亮云层导致白字无法辨认 |

验收样本文件：

- Job ID：`933dc541-ccaf-4e4b-8bf2-c0f2f9cadd6b`。
- 文件大小：2,362,143 bytes。
- SHA-256：`ede35bce788bbafd2c0dbda4bca8c0b56c30fbf1027b37db63d8c5ee92e8b1d8`。
- 独立使用Rasterio打开并逐块解码：GTiff、5490×5490、1波段、uint8、EPSG:32610、20×20 m、nodata=0，读取30,140,100个像元，值域2–10；大小和哈希与任务记录一致。
- 原资产：[SCL.tif](https://sentinel-cogs.s3.us-west-2.amazonaws.com/sentinel-s2-l2a-cogs/10/S/EG/2025/7/S2C_10SEG_20250707_0_L2A/SCL.tif)。这是一个样本的独立QA读取，不等于应用已内置科学正确性验证。

[机器验收记录](../prototype/qa/live-raster-acceptance.json) · [工作区截图](../prototype/qa/live-workspace-1440.png) · [成果库截图](../prototype/qa/live-library-desktop.png) · [窄屏检索](../prototype/qa/live-search-mobile.png) · [窄屏成果库](../prototype/qa/live-library-mobile.png)

独立读取脚本为 `scripts/inspect-downloaded-raster.py`，Rasterio仅是本机验收依赖，未捆绑到应用。应用运行时只检查文件签名、传输大小及计算SHA-256；不声称已检查完整栅格结构、空间精度或科学值。

## 工程检查与桌面产物

- `npm run verify`：依赖隔离、七张固定样本哈希、文档链接、拟议契约6个正例/14个反例、10项JavaScript测试和生产构建通过。
- `cargo test --locked --workspace`：8项runtime测试、3项desktop边界测试通过。
- `cargo clippy -p geod-runtime --all-targets -- -D warnings`通过。
- `npm audit`：0个已知漏洞；不表示整个产品经过安全认证。
- 在空依赖目录仅放置根manifest/lock后，`npm ci --prefer-offline --no-audit --no-fund --fetch-retries=0 --fetch-timeout=20000`成功安装23个包，锁文件未变。首次普通安装因npm registry连接重置中断，后续使用已有官方包缓存完成；没有替换注册源或放宽证书检查。
- `cargo build --locked -p geod-global-desktop --features custom-protocol`成功，最终包含本轮修复后的前端资源。
- Windows调试产物：`target/debug/geod-global-desktop.exe`，20,108,288 bytes，SHA-256 `cd4eab3168e37fc3e606bf5ad4b831e01fe4a7084e17418ea8c66d7c4f3d871e`。
- 桌面隐藏启动烟测：进程Responding、stderr为空，独立应用数据目录生成任务记录。原生窗口的完整操作和IPC端到端流程尚未验收；实际下载端到端证据来自浏览器加同一Rust核心。
- 国内版工作区修改与未跟踪文件清单和本轮开始一致。没有推送远程仓库、部署或发布安装包。

## 仍未完成

这条链路覆盖真实检索和完整资产获取。区域裁剪、重投影、科学指数、栅格渲染器、完整空间验证、字节级断点续传、其他五域、账户与支付、签名安装包及跨平台发行仍按完整工作包推进。当前Recipe继续是明确标识的设计对象，尚不能执行处理图。Runtime接口是内部v0.1接口，没有把06的拟议契约全部宣称为稳定API。

下一步应以下载后的真实GeoTIFF为输入，建立原生栅格读取/区域裁剪和对应验证器，然后接入成果预览与可执行Recipe。不能用模拟任务或缩略图代替这些验收。

技术参考：[Earth Search API](https://earth-search.aws.element84.com/v1/) · [STAC排序扩展](https://github.com/stac-api-extensions/sort) · [Tauri命令](https://v2.tauri.app/develop/calling-rust/) · [Tauri配置](https://v2.tauri.app/reference/config/)。
