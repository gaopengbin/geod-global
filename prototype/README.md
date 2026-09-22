# GeoD Global — 工作台前端

2026-09-22。沿用已认可的设计方向，已接入中英文界面、实时目录、Rust文件下载任务、真实二维栅格地图、SCL成果检查与裁剪、可执行配方、成果交付包与本地诊断。完整产品仍在实施；设计模拟继续单独标记。

## 本轮真实能力

- Live catalog直接查询Earth Search：可编辑WGS84范围、UTC日期、云量、分页，取消查询或恢复失败。
- 真实缩略图预览，只对相同源网格启用比较；实时缩略图不叠加未经验证的区域轮廓。
- Download source asset通过本机Rust服务或Tauri命令下载完整源资产，记录字节数、SHA-256、来源及本地路径，支持取消和从头重试。
- Tasks与My Data展示真实记录；原设计模拟收在独立折叠区。真实文件记录由Rust持久化，不依赖浏览器localStorage。
- Settings / 设置中的语言选项支持English和简体中文，即时切换并保存；日期按所选语言显示，UTC观测日期和内部标识符不变。
- My Data / 我的数据的成功SCL下载及派生成果可检查真实像元、元数据与分类统计，最长边768像素的最近邻预览来自本地GeoTIFF解码。
- 成功SCL下载可按工作区经纬度或手工源坐标预检矩形裁剪，显示真实像元窗口，再执行生成GeoTIFF及来源清单。编辑参数会使预检失效；任务支持取消和从头重试。
- Recipes的可执行配方由本地服务持久化，支持保存、JSON文本导入校验、导出审阅与重跑。导入不会自动执行；源任务与SHA-256必须匹配本机已完成的SCL。
- Workspace加载本地SCL源文件和派生结果，以真实UTM范围配准最近邻预览；最多4个同坐标系图层，支持显隐、透明度、定位、卸载和键盘平移／缩放。图层状态仅保留在本次页面会话，退出页面会释放地图与预览。
- 点击地图或读取地图中心点，向Rust请求原始分辨率文件的像元值、类别和列／行索引，每次重新校验源文件SHA-256。地图PNG不用于推算真实像元值。
- 地图绘制两个对角或输入源坐标矩形后，主动点击Review selected clip进入原有配方流程。选择本身不执行处理；真实预检仍是保存／运行的前提。
- My Data支持名称、场景或任务ID搜索，以及源文件／派生成果筛选。成功裁剪成果可准备ZIP交付包，包含已核验的GeoTIFF、来源清单、配方、说明与逐文件SHA-256；TIFF输入上限32 MiB，修改过的成果不会被静默导出。包内含配方名称和空间范围，分享前应审阅。
- Settings的Local diagnostics按需生成可复制的JSON支持报告，只包含版本、平台、能力、限额和任务状态计数；不包含路径、坐标、源URL或用户名称，也不自动上传。

完整启动和验证命令见[根README](../README.md)。浏览器模式需另开终端运行 `npm run runtime`；桌面版直接调用同一核心。

Agent通过独立的[本地MCP入口](../docs/mcp.md)复用相同任务和配方，不依靠操作浏览器来执行处理。默认只读，写入需启动参数显式启用；查看该说明中的任务完成／清理判定和数据目录所有权规则。

## 打开

本轮预览地址：<http://127.0.0.1:4317/>。服务停止后，在本目录运行：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\Start-Preview.ps1
```

脚本仅在 `127.0.0.1:4317` 提供已构建的 `dist/`，需要 Python。按 Ctrl+C 停止。也可以用任意静态 HTTP 服务打开 `dist/`。不要直接双击 HTML，浏览器会限制本地 JSON 读取。

跨机器重建：Node.js 22.12+，在仓库根目录执行 `npm ci`、`npm run build`、`npm run preview`。根 `package-lock.json` 固定依赖，Vite 只使用本仓库安装的依赖。已移除早期原型借用国内版依赖目录的临时设置；本仓库不需要国内版源码或本机路径。

## 保留的设计原型交互

- 八个入口：Explore、Workspace、My Data、Recipes、Tasks、Sources、Cloud、Settings。
- 六个数据分类，跨分类保留同一个样例区域；未接入分类明确展示计划状态。
- 七条真实 Sentinel-2 场景的本地日期、云量、ID 筛选与排序；空结果和来源失败的可恢复状态。
- 真实缩略图预览、同网格两日期卷帘比较、时间轴选择、缩放与区域轮廓。
- 导出参数审阅、明确标记的任务模拟，支持暂停、恢复、失败、重试和成果报告。
- 配方和模拟记录保存到浏览器 localStorage；配方 JSON、区域 GeoJSON 和报告可以打开文本预览并请求下载。
- 明暗主题、键盘命令搜索、原生模态框、窄屏布局。

`Workspace` 已使用独立真实地图，`Explore` 保留目录发现和场景预览流程。当前地图仅支持已有SCL栅格的同坐标系叠加与矩形选择，尚不是通用多数据类型编辑器。Cloud 是提案展示，没有账户、付费或同步服务。

## 数据真实性

`public/samples/earth-search-response.json` 是实际获取的 STAC 检索响应，`manifest.json` 保留查询、采集日期、获取时间、原资产链接、SHA-256 和栅格网格元数据。七张 JPEG 是原始提供商缩略图，没有生成或冒充卫星数据。

固定样例：San Francisco Bay，WGS84 bbox `[-122.55,37.68,-122.32,37.84]`；查询时间 2025-06-01 至 2025-06-30。云量是整景统计，不是区域内云量。343×343 缩略图只能作概览，界面的 10 m 指原始 RGB 资产分辨率。区域叠加由原资产 UTM 10N 网格换算，用于定位示意，不作为科学空间验收。

来源：[Earth Search 查询](https://earth-search.aws.element84.com/v1/search?collections=sentinel-2-l2a&bbox=-122.55,37.68,-122.32,37.84&datetime=2025-06-01T00:00:00Z/2025-06-30T23:59:59Z&limit=8)、[AWS 数据集与条款索引](https://registry.opendata.aws/sentinel-2-l2a-cogs/)。署名：Contains Copernicus Sentinel data (2025). Earth Search / Element 84.

字体 Inter 随包附带，许可在 `public/fonts/LICENSE-Inter.txt`。样本模式浏览本地记录；Live catalog及源文件下载访问Earth Search与Sentinel COG存储。不发送分析事件；打开来源链接会访问相应网站。

Workspace的地理配准来自本地GeoTIFF元数据，与上述样本缩略图定位示意分开。地图不请求在线底图，也不对显示图层或输出像元作跨坐标系重投影。OpenLayers和proj4版本由根锁文件固定，发行许可收集流程见[Windows打包说明](../docs/releases/windows-packaging.md)。

## 能力边界

实时检索、完整源文件下载及SCL矩形裁剪已实现。重投影、多边形掩膜、通用多波段、科学指数、3D renderer、云账户、支付与字节级断点续传尚未实现。重启后运行中的任务标为interrupted，可手动从头重试。设计模拟任务仍只产生 **simulation report**，不能当作栅格成果。

原型 JSON 使用 `design-prototype/v1`，不能交给当前 CLI 或把它当作 `contracts/` 的拟议 Core 契约。后续转换需完成对象 ID、固定资产版本、授权策略、资源预算、操作图和验证规则映射。

JSON提供完整文本审阅窗口；旧设计原型的blob下载完成仍未确认。真实成果ZIP已通过浏览器下载到Windows Downloads目录，并与Rust受管导出文件核对SHA-256一致（详见附件12）。桌面通过显示文件位置访问交付包；原生GUI操作需要单独验收。

## 验收

见 [07 原型与验收记录](../GeoD-Global-Spec/07-Prototype-and-Validation.md)、[09 真实检索与下载](../GeoD-Global-Spec/09-Live-Catalog-and-Downloads.md)、[10 多语言与栅格检查](../GeoD-Global-Spec/10-Languages-and-Raster-Inspection.md)、[11 可执行处理与配方](../GeoD-Global-Spec/11-Executable-Processing-and-Recipes.md)和[12 工作区、Agent与交付](../GeoD-Global-Spec/12-Workspace-Agent-and-Distribution.md)。截图在 `qa/`，构建产物在 `dist/`。桌面重新构建后嵌入本前端；不会修改国内版应用。

Windows构建脚本和手动CI只提供打包能力，不代表已有签名发行或远程CI成功。发行构建、桌面原生GUI／IPC、安装／卸载、干净机器和浏览器验收结论分别以附件12及[发行文档](../docs/releases/windows-packaging.md)为准。
