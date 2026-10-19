<div align="center">

<img src="prototype/public/brand/geod-symbol.png" width="80" alt="GeoD Global">

# GeoD Global
**找到地理数据，确认完整任务，把成果留在本地。**

[![Windows](https://img.shields.io/badge/Windows-10%20%2F%2011%20x64-2563eb?style=flat-square)](docs/releases/WINDOWS-README.md) [![Release](https://img.shields.io/github/v/release/gaopengbin/geod-global?style=flat-square)](https://github.com/gaopengbin/geod-global/releases/latest) [![GPL](https://img.shields.io/badge/license-GPL--3.0-0f766e?style=flat-square)](LICENSE) [![Checks](https://github.com/gaopengbin/geod-global/actions/workflows/check.yml/badge.svg)](https://github.com/gaopengbin/geod-global/actions/workflows/check.yml)

**[下载 0.2.0](https://github.com/gaopengbin/geod-global/releases/tag/v0.2.0)** · **[官网](https://geod-global.laogao.xyz)** · **[问题反馈](https://github.com/gaopengbin/geod-global/issues)**

[English](README.md) · 简体中文
</div>

面向 Windows 的地理数据工作台，把数据源选择、地图探索、下载确认和本地成果组织在一起。
可以手动探索，也可以连接自己的模型，让 Agent 准备待确认的任务。
**本地使用和自接模型无需注册 Global 账号。**

<img src="docs/images/sentinel2-desktop.jpg" width="100%" alt="GeoD Global 真实桌面地图">

<sub>2026 年 9 月 30 日原生桌面历史截图，影像由 Earth Search 提供。画面是远程预览，不代表下载完成。[图片来源](docs/images/README.md)。</sub>

## 从选数据到看成果，一条流程

| 选来源 | 做探索 | 确认任务 | 下载执行 | 查看成果 |
| :--- | :--- | :--- | :--- | :--- |
| 分类产品与访问条件 | 紧凑列表、多选、地图预览 | 核对来源、区域、文件和保存位置 | 持久任务与明确的完成状态 | 本地文件、来源、校验和原始像元 |

- **133 项目录，17 个分类。** 涵盖影像、高程、专题栅格、矢量、服务和本地格式。**110 Ready、2 Auth、21 Planned** 表示连接器状态，不等于卫星数量或所有产品均已实测。
- **先显示最新 100 条。** 更多结果按需加载；NAIP 优先最近拍摄年份，切换区域不会自动拉完整历史目录。
- **自己的模型，自己的任务。** Agent 使用受限原生工具、任务确认和持久作业。首页会话与探索侧栏保留各自场景。
- **地图与成果对应真实数据。** 多图层预览、原始像元读取，以及已支持的兼容网格裁剪和拼接；地图服务导出与原始科学波段保留各自含义。

## 开始使用

1. 在 [Releases](https://github.com/gaopengbin/geod-global/releases/latest) 下载 **Windows x64 安装 EXE**，或完整解压免安装 ZIP。
2. 设备需要 Microsoft Edge WebView2 Evergreen；无需另装 Node、Rust、Python 或系统 GDAL。
3. 进入“探索”选择来源和区域，检索、多选并确认下载。需要 Agent 时，在设置中连接自己的模型。

程序文件**尚未签名**。请核对 SHA256SUMS.txt 和发行清单，校验值不能替代 Windows 发布者签名。
升级前从托盘退出；当前使用手动下载升级。卸载保留用户数据。[Windows 使用说明](docs/releases/WINDOWS-README.md)

## 授权、隐私和支持范围

数据源账号与软件账号相互独立。受保护产品需要自己的有效授权；
Earth Engine 使用自己的项目，可能消耗该项目额度。Planned 表示仍未完整接入。

匿名使用统计**默认开启**，设置中可关闭，已保存的关闭选择会保留。
不采集对话、模型密钥、账号资料、精确位置、影像编号或文件路径。[统计范围与保留期限](docs/usage-analytics.md)。
模型请求访问你配置的提供方，数据请求访问相应数据源。

当前聚焦 **2D 地理数据**，包含高程栅格。不包含托管模型、正式付费服务、云同步、任意跨网格处理或其他系统安装包。
[数据源清单](docs/releases/source-scope.json) · [对接状态](docs/provider-integration-status.md) · [0.2.0 发行说明](docs/releases/0.2.0.md)

## 开发与贡献

开发环境：Windows x64、Node 22.13+、npm 10+、Rust 1.91.1+、Visual Studio C++ Build Tools 和 WebView2。
在仓库根目录依次运行 npm ci、npm run agent:prepare、npm run desktop:dev。

[开发指南](docs/development-guide.md) · [Agent 架构](docs/agent.md) · [CLI / MCP](docs/mcp.md) ·
[处理示例](docs/workflows/clip-sentinel-scl.md) · [Windows 打包](docs/releases/windows-packaging.md)

欢迎通过 [Issues](https://github.com/gaopengbin/geod-global/issues) 提交真实需求和可复现问题。
请附产品、时间范围、步骤和版本，不要提交凭据或客户私有数据。

## 许可

第一方代码采用 **[GPL-3.0-only](LICENSE)**，© 2026 Gao Pengbin。
遵守 GPL 义务即可商业使用，也可洽谈[单独商业授权](COMMERCIAL-LICENSING.md)。数据集和组件保留各自许可。
