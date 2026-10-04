# 开发与验收参考

[English development guide](development-guide.md) · [中文概览](../README.zh-CN.md)

本指南对应 0.1.0-rc.2 源码和共用桌面 / 运行时核心。产品支持范围、真实数据证据和待验收事项以[数据源说明](providers.md)、[公开预览](explore-public-previews.md)及[候选说明](releases/0.1.0-rc.2.md)为准。

## 本地开发

准备 Node.js 22.12+、npm 10+、Rust 1.91.1+，契约检查另需 Python 3.12。Windows 原生开发需要 MSVC 和 WebView2，见[桌面环境](../src-tauri/README.md)。在仓库根目录安装锁定依赖，prototype 不拥有单独的 npm 包。

| 命令 | 用途 |
| --- | --- |
| `npm ci` | 安装锁定的前端依赖 |
| `npm run desktop:dev` | 启动桌面开发版，直接使用 Rust 核心 |
| `npm run dev` | 浏览器调试界面，监听 127.0.0.1:4317 |
| `npm run runtime` | 浏览器调试所需文件 / 任务服务，监听 127.0.0.1:4318 |
| `npm run desktop:build` | 生成调试程序，不创建安装包 |
| `npm run build` / `npm run preview` | 构建和预览生产前端 |
| `python -m pip install -r requirements-dev.txt` | 安装契约检查依赖，建议使用虚拟环境 |
| `npm run verify` | 检查依赖隔离、样本、契约、配方、前端测试和生产构建 |
| `cargo test --locked --workspace --features geod-global-desktop/custom-protocol` | 运行原生核心与桌面测试 |
| `cargo fmt --all --check` | Rust 格式检查 |
| `cargo clippy --locked --workspace --all-targets --features geod-global-desktop/custom-protocol -- -D warnings` | 原生代码检查 |
| `python scripts/package-windows.test.py` / `python scripts/release.test.py` | 发布包和来源校验测试 |

浏览器服务的数据位于 .geod-global，桌面数据使用独立应用标识 xyz.laogao.geod.global。桌面无需单独启动浏览器服务。重启后未完成任务可重试；符合条件的公开原文件保留经过校验的续传记录，其他任务从头开始。

## 能力与证据

- 公开目录使用共用区域、分页和工程流程，日期、云量、轨道、极化及合成时段筛选按产品提供。测试样本与实时目录分别记录。
- 原文件检查核对受管路径、SHA-256、支持的网格和像元类型。工作区最多叠加四个同坐标系图层，返回原始像元，不把显示颜色当成科学值。
- 裁剪、同网格拼接、科学 RGB 和质量筛选按支持产品执行；通用重投影和任意科学格式处理尚未提供。
- 普通裁剪 ZIP 的 TIFF 输入上限 32 MiB；科学 RGB 使用独立的 512 MiB 有界交付。包内保留来源、配方和校验值。
- 持久缩略图、任务重试、托盘后台运行和面板宽度调整使用同一桌面核心。无头前端或启动检查不能代替原生窗口、托盘和干净设备验收。
- CLI 与 MCP 复用实际任务、像元和配方能力。MCP 默认读取 / 预检，写入需显式启用；[MCP 文档](mcp.md)列出当前工具和连接方式。软件内对话 Agent 仍待接入。
- 矢量、地图服务、离线瓦片和有界三维工作流各有独立范围，详见[数据源状态](provider-integration-status.md)。NASA / Copernicus 账号入口已提供，受保护原文件仍待真实账号验收。
- 本地诊断按需生成脱敏版本、能力和任务计数，不自动上传。渲染瓦片、在线预览和已完成原文件下载是不同能力。

## 发布构建

安装包和便携包按[Windows 打包说明](releases/windows-packaging.md)从干净提交生成。第一方代码采用 GPL-3.0-only，包内包含完整协议、精确提交的源码档案及校验值；第三方许可和适用源码档案单独保留。源码包含锁定依赖清单和构建脚本，未包含任务数据、凭据和生成的本地输出。

[发布自动化](releases/automation.md)执行跨平台检查、Windows 构建、产物上传及服务器副本校验，再公开预发布。[GitHub Releases](https://github.com/gaopengbin/geod-global/releases)显示实际发布状态。当前候选未签名，没有自动更新；干净设备安装 / 升级 / 卸载与原生界面体验分别验收。

## 仓库边界

GeoD Global 是独立产品仓库。国内 GeoD 不作为本工程的运行时依赖；不得从相邻工程解析源码、程序、node_modules 或本地兜底路径。界面延续共用 Beautiful UI 组件层，样本和真实下载 / 处理证据保持区分。

[GPLv3 协议](../LICENSE) · [商业许可询问](../COMMERCIAL-LICENSING.md) · [截图与插画来源](images/README.md) · [完整产品规划](../GeoD-Global-Spec/00-README.md)
