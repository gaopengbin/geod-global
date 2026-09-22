# GeoD Global

独立海外桌面产品仓库。产品目标是围绕同一区域发现、预览、比较、获取、处理和导出空间数据，并保留来源及可重复工作流。

**当前状态：独立工程已接入中英文界面、Earth Search 实时检索、Rust 文件下载与原生 SCL 栅格检查。Tauri 桌面壳与浏览器调试入口复用同一执行核心。完整产品规格仍在逐步实施。**

已实现按WGS84范围、UTC日期与云量检索 Sentinel-2，预览和比较兼容网格的场景，下载原始SCL/真彩色GeoTIFF或JPEG缩略图，取消、从头重试、持久化任务及查看成果来源与SHA-256。样本目录、设计模拟和真实任务有明确区别。当前下载整景原始资产；裁剪、重投影、科学计算和断点续传尚未实现。

在 **Settings / 设置 → Language / 语言** 切换 English / 简体中文，选择会保留。**My Data / 我的数据 → 检查栅格** 可读取已完成的SCL下载，重新校验SHA-256，并显示原始栅格元数据、真实像元预览和整景分类统计。当前读取支持单波段UInt8 SCL、WGS84 UTM北/南网格；应用无需系统GDAL或Python。具体范围和实际验收见[多语言与栅格检查](GeoD-Global-Spec/10-Languages-and-Raster-Inspection.md)。

## 仓库边界

- 海外版拥有自己的 Git 历史、依赖锁文件、配置、构建和发布流程。
- 国内版是独立项目；本仓库不包含国内版工作树，也不读取它的源码或依赖来启动。
- 以后需要复用核心能力时，使用经过审查、有明确版本的公共包或库；不使用跨仓路径、软链接或本机依赖回退。
- 最终形态为桌面主产品、配套公开网站及可选 Web 协作服务。当前浏览器页面用于设计验收，不代表纯 Web 产品改向。

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

`verify` 检查四个关键依赖的真实解析路径、七张真实样本的 SHA-256、规格相对链接、拟议契约正反例、多语言、目录与下载/栅格客户端测试和 Vite 构建。`npm run test:runtime` 运行Rust下载、栅格读取与HTTP边界测试；`npm run verify:all` 同时运行两部分。契约草案不是当前生产API；本轮运行时接口见 [runtime说明](crates/geod-runtime/README.md)。

GitHub Actions 已配置 Windows/Linux 检查，但在没有推送并完成运行前，不代表远程 CI 已通过。

## 目录

```text
GeoD-Global-Spec/       完整产品规格、代码参考审计、决策及拟议契约
prototype/             已认可视觉方向的交互原型
  src/                 React 页面与样式
  public/              真实场景快照、缩略图及字体许可
  qa/                  原型视觉验收截图
scripts/               独立依赖与样本检查
crates/geod-runtime/    持久化下载任务与loopback调试服务
src-tauri/             独立桌面壳，直接调用同一Rust核心
.github/workflows/     独立构建检查
```

[规格导航](GeoD-Global-Spec/00-README.md) · [原型使用说明](prototype/README.md) · [仓库隔离决定](GeoD-Global-Spec/08-Repository-Boundary.md)

## 样本、许可和发布状态

样本模式使用本地保存的七条 Sentinel-2 元数据和提供商 JPEG 缩略图。来源、原始链接及校验值见 `prototype/public/samples/manifest.json`；Inter 字体许可随包保存，中文使用平台字体回退。Live catalog会访问Earth Search，预览远程缩略图；点击下载会获取选定的真实源文件。下载阶段检查传输大小、文件签名及SHA-256；单独执行SCL检查时还会读取地理标签、解码像元并统计类别。两者均不代表分类精度认证，其他栅格类型尚未实现通用读取。

新产品代码的对外许可和商业包装尚待决定，根包以 `private: true` / `UNLICENSED` 防止被误当作已发布公共软件包。这不改变国内版或第三方资产已有权利。今后引入共享库必须保留其许可通知。

本地仓库初始化和提交不等于创建或发布了 GitHub 仓库。远程地址为空时，内容仍仅保存在本机。
