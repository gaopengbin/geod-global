# GeoD Global

独立海外桌面产品仓库。产品目标是围绕同一区域发现、预览、比较、获取、处理和导出空间数据，并保留来源及可重复工作流。

**当前状态：独立工程已接入中英文界面、Earth Search 实时检索、Rust 文件下载、真实二维栅格工作区、SCL 像元检查与矩形裁剪、可执行配方、成果交付包、CLI 和本地 MCP。Tauri 桌面壳与浏览器调试入口复用同一执行核心。完整产品规格仍在逐步实施。**

已实现通过离线参考地图绘制WGS84矩形选区，或输入精确坐标，再按范围、UTC日期与云量检索 Sentinel-2；也可预览和比较兼容网格的场景，下载原始SCL/真彩色GeoTIFF或JPEG缩略图，取消、从头重试、持久化任务及查看成果来源与SHA-256。样本目录、设计模拟和真实任务有明确区别。选区用于目录检索，源资产下载仍为整景；成功下载的SCL可以另行裁剪为派生GeoTIFF。重投影、通用多波段处理、科学计算和字节续传尚未实现。

在 **Explore / 探索** 点击区域名称，或切到 **Live catalog / 实时目录** 点击 **Draw area on map / 在地图上选区**。地图可平移、缩放并切换全球视图；点击“绘制矩形”后从一角拖到另一角，再点击“搜索此区域”。所选坐标会写入检索条件并立即查询真实目录。也可编辑西、南、东、北四个WGS84坐标；非法范围会阻止提交。该地图仅作选区参考，场景缩略图不用于地理选区。

在 **Settings / 设置 → Language / 语言** 切换 English / 简体中文，选择会保留。**My Data / 我的数据 → 检查栅格** 可读取已完成的SCL下载及派生成果，重新校验SHA-256，并显示栅格元数据、真实像元预览和当前文件的分类统计。当前读取支持单波段UInt8 SCL、WGS84 UTM北/南网格；应用无需系统GDAL或Python。具体范围和实际验收见[多语言与栅格检查](GeoD-Global-Spec/10-Languages-and-Raster-Inspection.md)。

**我的数据 → 裁剪栅格** 可使用工作区经纬度范围或手工源坐标，在真实预检后保存配方或执行。输出保留源像元、坐标系和分辨率，并附带独立JSON来源清单。**Recipes / 配方** 支持保存、导入、审阅及重跑；同一流程也可通过CLI执行。见[处理与配方验收](GeoD-Global-Spec/11-Executable-Processing-and-Recipes.md)和[英文操作教程](docs/workflows/clip-sentinel-scl.md)。

**Workspace / 工作区** 可加载最多4个同坐标系的本地SCL源文件或裁剪结果，按原始UTM网格叠加预览，支持显隐、透明度、定位和卸载。点击地图会从经过校验的原始分辨率文件读取像元；绘制矩形或填写源坐标后，需主动进入配方预检才能执行裁剪。地图不请求在线底图，不对像元重投影；当前图层选择仅保留在打开的页面会话中。

**我的数据** 支持按名称、场景或任务ID搜索，以及下载源文件／派生成果筛选。成功的裁剪成果可准备ZIP交付包，包含GeoTIFF、来源清单、配方、说明和逐文件校验值；仅接受已核验的托管成果，TIFF上限32 MiB。交付包包含配方名称及空间范围，分享前需审阅。**设置 → 本地诊断** 按需生成版本、能力和任务计数报告，不含本机路径、坐标、源URL或用户名称，不自动上传。新增流程与验收边界见[工作区、Agent与交付记录](GeoD-Global-Spec/12-Workspace-Agent-and-Distribution.md)。

本地Agent可通过 `geod-runtime serve-mcp` 调用同一任务、像元与配方核心，默认只开放读取和预检；写入需启动参数显式启用。独立目录模式与连接已有loopback服务模式分别管理任务生命周期，具体配置和完成判定见[MCP说明](docs/mcp.md)。

## 仓库边界

- 海外版拥有自己的 Git 历史、依赖锁文件、配置、构建和发布流程。
- 国内版是独立项目；本仓库不包含国内版工作树，也不读取它的源码或依赖来启动。
- 以后需要复用核心能力时，使用经过审查、有明确版本的公共包或库；不使用跨仓路径、软链接或本机依赖回退。
- 最终形态为桌面主产品、配套公开网站及可选 Web 协作服务。当前浏览器页面是本地调试与交互验收入口，不代表纯 Web 产品改向。

## 快速启动

Node.js 22.12+、npm 10+；在仓库根目录执行：

```sh
npm ci
npm run dev
```

打开 <http://127.0.0.1:4317/>。实时目录无需登录；浏览器中的真实文件下载另开一个终端运行（需要Rust 1.91.1+）：

```sh
npm run runtime
```

服务只监听 `127.0.0.1:4318`，浏览器来源固定为 `http://127.0.0.1:4317`，任务与文件保存于本仓忽略目录 `.geod-global/`。仅允许已支持的 Sentinel COG 公共资产，每文件最多512 MiB。关闭并重新启动服务后，未完成任务标为 interrupted，可手动从头重试。

桌面开发与本机调试构建：

```sh
npm run desktop:dev
npm run desktop:build
```

桌面版直接调用Rust核心，不需要单独启动4318服务，数据保存在独立应用ID `xyz.laogao.geod.global` 的本地应用数据目录。`desktop:build` 生成调试版可执行文件，尚不是签名发行版或安装包；平台依赖和命令见 [桌面说明](src-tauri/README.md)。

Windows便携ZIP与NSIS安装器已从干净提交生成，逐文件校验、实际包CLI裁剪流程和桌面进程启动检查通过；文件位置与SHA-256见[本地评估包验收](docs/releases/2026-09-22-windows-artifact-acceptance.md)。重建、第三方许可收集和校验流程见[Windows打包说明](docs/releases/windows-packaging.md)。这些是未签名的本地评估产物；原生桌面交互、干净机器安装／卸载、签名和公开发布仍需分别完成。

生产资源预览：

```sh
npm run build
npm run preview
```

这几个命令仅需要本仓库，不依赖另一个 GeoD checkout。

## 验证

契约校验另需 Python 3.12（建议使用虚拟环境）：

```sh
python -m pip install -r requirements-dev.txt
npm run verify
```

`verify` 检查四个关键依赖的真实解析路径、七张真实样本的 SHA-256、规格相对链接、拟议契约和当前可执行配方Schema正反例、多语言、目录与下载/栅格/处理客户端、地图几何测试和 Vite 构建。`npm run test:runtime` 运行Rust下载、栅格读取/裁剪、配方持久化、像元、交付包、诊断、MCP与HTTP边界测试；`npm run verify:all` 同时运行两部分。完整工作区与桌面测试可运行 `cargo test --locked --workspace`。契约草案不是当前生产API；本轮运行时接口见 [runtime说明](crates/geod-runtime/README.md)。

GitHub Actions 已配置分支／PR 的 Windows/Linux 检查、手动 Windows ZIP／NSIS 构建和 `v*` 标签预发布；发布会校验版本、干净提交、构建来源及服务器上的产物哈希。使用方式与失败恢复见[提交和发布自动化](docs/releases/automation.md)。配置存在不代表远程 CI 已通过，应以实际运行记录为准。

## 目录

```text
GeoD-Global-Spec/       完整产品规格、代码参考审计、决策及拟议契约
prototype/             桌面共用前端、本地调试入口与独立标记的设计预览
  src/                 React 页面与样式
  public/              真实场景快照、缩略图及字体许可
  qa/                  原型视觉验收截图
scripts/               独立依赖与样本检查
crates/geod-runtime/    持久化下载/处理/配方/交付、CLI、MCP与loopback服务
schemas/               当前可执行配方Schema（区别于规格草案）
examples/              实际下载请求与固定本地输入的配方示例
docs/                  英文操作教程、MCP接入与发行构建说明
src-tauri/             独立桌面壳，直接调用同一Rust核心
.github/workflows/     独立构建检查
```

[规格导航](GeoD-Global-Spec/00-README.md) · [原型使用说明](prototype/README.md) · [仓库隔离决定](GeoD-Global-Spec/08-Repository-Boundary.md)

## 样本、许可和发布状态

样本模式使用本地保存的七条 Sentinel-2 元数据和提供商 JPEG 缩略图。来源、原始链接及校验值见 `prototype/public/samples/manifest.json`；Inter 字体许可随包保存，中文使用平台字体回退。Live catalog会访问Earth Search，预览远程缩略图；点击下载会获取选定的真实源文件。下载阶段检查传输大小、文件签名及SHA-256；单独执行SCL检查时还会读取地理标签、解码像元并统计类别。两者均不代表分类精度认证，其他栅格类型尚未实现通用读取。

新产品代码的对外许可和商业包装尚待决定，根包以 `private: true` / `UNLICENSED` 防止被误当作已发布公共软件包。这不改变国内版或第三方资产已有权利。今后引入共享库必须保留其许可通知。

独立远程仓库为 [gaopengbin/geod-global](https://github.com/gaopengbin/geod-global)，创建时为私有仓库。GitHub 源码访问、Actions 产物与 Release 下载均受仓库权限控制；公开发布和更改源码许可是另外的决定。
