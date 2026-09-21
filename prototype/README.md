# GeoD Global — 可交互设计原型

2026-09-21。用于审阅完整产品的信息架构和代表性交互，不是已接入 Rust Core 的桌面发行版。

## 打开

本轮预览地址：<http://127.0.0.1:4317/>。服务停止后，在本目录运行：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\Start-Preview.ps1
```

脚本仅在 `127.0.0.1:4317` 提供已构建的 `dist/`，需要 Python。按 Ctrl+C 停止。也可以用任意静态 HTTP 服务打开 `dist/`。不要直接双击 HTML，浏览器会限制本地 JSON 读取。

跨机器重建：Node.js 22.12+，在仓库根目录执行 `npm ci`、`npm run build`、`npm run preview`。根 `package-lock.json` 固定依赖，Vite 只使用本仓库安装的依赖。已移除早期原型借用国内版依赖目录的临时设置；本仓库不需要国内版源码或本机路径。

## 已实现的原型交互

- 八个入口：Explore、Workspace、My Data、Recipes、Tasks、Sources、Cloud、Settings。
- 六个数据分类，跨分类保留同一个样例区域；未接入分类明确展示计划状态。
- 七条真实 Sentinel-2 场景的本地日期、云量、ID 筛选与排序；空结果和来源失败的可恢复状态。
- 真实缩略图预览、同网格两日期卷帘比较、时间轴选择、缩放与区域轮廓。
- 导出参数审阅、明确标记的任务模拟，支持暂停、恢复、失败、重试和成果报告。
- 配方和模拟记录保存到浏览器 localStorage；配方 JSON、区域 GeoJSON 和报告可以打开文本预览并请求下载。
- 明暗主题、键盘命令搜索、原生模态框、窄屏布局。

`Workspace` 当前复用探索面板，仅用于验证统一区域与工作上下文；独立多图层编辑器尚未完成。Cloud 是提案展示，没有账户、付费或同步服务。

## 数据真实性

`public/samples/earth-search-response.json` 是实际获取的 STAC 检索响应，`manifest.json` 保留查询、采集日期、获取时间、原资产链接、SHA-256 和栅格网格元数据。七张 JPEG 是原始提供商缩略图，没有生成或冒充卫星数据。

固定样例：San Francisco Bay，WGS84 bbox `[-122.55,37.68,-122.32,37.84]`；查询时间 2025-06-01 至 2025-06-30。云量是整景统计，不是区域内云量。343×343 缩略图只能作概览，界面的 10 m 指原始 RGB 资产分辨率。区域叠加由原资产 UTM 10N 网格换算，用于定位示意，不作为科学空间验收。

来源：[Earth Search 查询](https://earth-search.aws.element84.com/v1/search?collections=sentinel-2-l2a&bbox=-122.55,37.68,-122.32,37.84&datetime=2025-06-01T00:00:00Z/2025-06-30T23:59:59Z&limit=8)、[AWS 数据集与条款索引](https://registry.opendata.aws/sentinel-2-l2a-cogs/)。署名：Contains Copernicus Sentinel data (2025). Earth Search / Element 84.

字体 Inter 随包附带，许可在 `public/fonts/LICENSE-Inter.txt`。正常操作只访问本地样例，不发送分析事件；打开来源链接会访问相应网站。

## 能力边界

不包含实时检索、GeoTIFF/COG 实际下载、重投影、科学指数、真实故障恢复、3D renderer、云账户或支付。任务完成只产生 **simulation report**，没有栅格成果。

原型 JSON 使用 `design-prototype/v1`，不能交给当前 CLI 或把它当作 `contracts/` 的拟议 Core 契约。后续转换需完成对象 ID、固定资产版本、授权策略、资源预算、操作图和验证规则映射。

内置浏览器的 blob 文件下载事件在本轮未确认。为此提供完整 JSON 文本窗口；常规浏览器的下载保存仍需验证。本轮验证了文本可读取且是有效 JSON，不宣称已验证文件落盘。

## 验收

见 [07 原型与验收记录](../GeoD-Global-Spec/07-Prototype-and-Validation.md)。截图在 `qa/`，构建产物在 `dist/`。`src/` 是原型代码，不会自动修改现有桌面应用。
