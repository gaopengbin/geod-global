<div align="center">

<img src="docs/images/readme-cover.svg" alt="GeoD Global 空间数据工作区，Windows 开发预览，GPLv3。" width="100%">

### 从一景影像，到可用的本地数据。

面向 Windows 的空间数据工作区：发现影像、检查源像元、
准备支持的栅格，并把来源信息留在项目里。

**[查看工作流](#从发现到交付)** · **[源码启动](#从源码启动)** · **[反馈任务](https://github.com/gaopengbin/geod-global/issues)**

[English](README.md) · 简体中文

</div>

---

<img src="docs/images/sentinel2-desktop.jpg" alt="真实英文 Windows 桌面截图：旧金山湾 Sentinel-2 影像、场景列表、采集信息与地理预览。" width="100%">

<sub>2026-09-30 的真实英文 Windows 桌面截图。Copernicus Sentinel data (2026) · Earth Search · Natural Earth overview。图中较新的开发界面领先于当前公开源码快照；展示的是远程影像预览，不代表已完成下载或导出。</sub>

## 为分析前的数据准备而做

找到一景影像只是开始。GeoD Global 把数据发现、源文件、栅格检查和可重复的准备流程放进本地项目，方便你带着清晰的来源继续在 GIS 或科研工具中分析。

| 发现与比较 | 检查与准备 | 保存与重复 |
| :--- | :--- | :--- |
| 按区域、日期、云量检索支持的 Sentinel-2 场景，查看元数据和兼容预览。 | 读取原始 SCL 像元，检查本地栅格，按源网格做矩形裁剪。 | 把源身份、配方、来源清单与 SHA-256 校验值留在成果旁。 |

## 从发现到交付

**01 / 找到场景** → **02 / 获取源文件** → **03 / 检查像元** → **04 / 审阅裁剪** → **05 / 带来源交付**

1. **发现：** 通过支持的 Earth Search 目录检索 Sentinel-2，核对覆盖范围、时间和源资产。
2. **下载：** 获取支持的整景 SCL／真彩色 GeoTIFF 或 JPEG 缩略图；任务本地持久化，中断后可从头重试。
3. **检查：** 读取原始 WGS84 UTM 网格中的 UInt8 SCL 像元；二维工作区最多叠加 4 个同坐标系图层。
4. **准备：** 在真实预检后执行 SCL 矩形裁剪，保存配方，再从界面或 CLI 重跑。
5. **交付：** 对已核验的托管裁剪成果准备 ZIP，包含 GeoTIFF、来源、配方、说明和校验值。当前交付包的 TIFF 输入上限为 32 MiB。

[阅读可执行的 SCL 工作流 →](docs/workflows/clip-sentinel-scl.md)

## 当前范围

**源码已公开，Windows 正式发行仍在准备。** 当前分支是较早的开发快照，更新的桌面功能在另行开发和验收；上方截图已注明版本差异。

| 公开快照已具备 | 此快照尚未具备 |
| :--- | :--- |
| Sentinel-2 发现与支持的公共资产下载 | 通用多波段处理与任意重投影 |
| SCL 像元检查与源网格矩形裁剪 | 多边形掩膜、通用科学计算、三维 |
| 本地项目、配方、CLI 与有限范围 MCP | 云端同步与经过真实授权验收的受保护数据工作流 |
| 中英文界面、明暗外观 | 已支持的公开安装包、签名与干净机器发行验收 |

旧 v0.1.0 预览 Release 已转为草稿，历史未签名 CI 产物属于评估构建。独立英文官网和可公开使用的预约入口仍在准备，尚无公网地址。

## 从源码启动

准备 **Node.js 22.12+**、**npm 10+**、**Rust 1.91.1+**，在仓库根目录运行：

```sh
git clone https://github.com/gaopengbin/geod-global.git
cd geod-global
npm ci
npm run dev
```

打开 **http://127.0.0.1:4317/**。浏览器调试界面的真实文件下载另开终端启动：

```sh
npm run runtime
```

服务仅监听 `127.0.0.1:4318`，任务与文件位于 `.geod-global/`。目录浏览无需 GeoD 账号；数据提供商的权利和授权要求仍适用。

<details>
<summary><strong>原生桌面、构建与验证</strong></summary>

平台依赖见[桌面说明](src-tauri/README.md)。桌面版直接调用 Rust 核心，无需单独启动浏览器运行时。

```sh
npm run desktop:dev
npm run desktop:build
```

`desktop:build` 生成调试可执行文件，不是已签名的公开安装包。前端生产资源预览：

```sh
npm run build
npm run preview
```

契约检查另需 Python 3.12，建议使用虚拟环境：

```sh
python -m pip install -r requirements-dev.txt
npm run verify
npm run test:runtime
```

`npm run verify:all` 合并前端／契约检查与运行时测试，`cargo test --locked --workspace` 包含桌面 crate。这些是验证命令，不表示当前远程 CI 均已通过。

完整参考：[开发与验收参考](docs/development-guide.zh-CN.md) · [English development guide](docs/development-guide.md)。

</details>

## 同一个核心，多种使用方式

**桌面工作区**用于交互准备，**CLI**用于重复配方，**本地 MCP**用于支持的工具调用，复用同一套 Rust 任务和栅格核心。

入口 `geod-runtime serve-mcp` 默认只开放读取与预检，写入需要显式启用。[连接方式与生命周期 →](docs/mcp.md)

## 文档导航

| 从这里开始 | 深入了解 |
| :--- | :--- |
| [SCL 裁剪工作流](docs/workflows/clip-sentinel-scl.md) | [多语言与栅格检查](GeoD-Global-Spec/10-Languages-and-Raster-Inspection.md) |
| [本地 MCP](docs/mcp.md) | [处理与配方](GeoD-Global-Spec/11-Executable-Processing-and-Recipes.md) |
| [开发与验收参考](docs/development-guide.zh-CN.md) | [工作区与交付证据](GeoD-Global-Spec/12-Workspace-Agent-and-Distribution.md) |
| [Windows 打包](docs/releases/windows-packaging.md) | [完整产品规格](GeoD-Global-Spec/00-README.md) |

## 反馈与贡献

欢迎描述一个近期真实任务：用了哪个场景或栅格、需要什么本地结果、在哪一步遇到困难。[创建 Issue](https://github.com/gaopengbin/geod-global/issues)，或[关注开发者 Bluesky](https://bsky.app/profile/laogao98.bsky.social)。

代码改动先通过 Issue 对齐范围，区分样本与真实下载／处理证据，保留第三方归属信息。公开反馈中请省略私有客户数据、密钥与敏感坐标。

## 协议

自有项目代码采用 **[GPL-3.0-only](LICENSE)**，© 2026 Gao Pengbin。GPL 允许商用；分发覆盖程序时须履行相应源码与许可义务。[商业授权可另议](COMMERCIAL-LICENSING.md)，当前不自动授予 GPL 之外的例外。

第三方软件、字体、影像与资产保留各自条款。[截图和插画来源](docs/images/README.md)。

---

<div align="center"><sub>GeoD Global · Gao Pengbin 独立开发 · 留下源文件，也留下来源。</sub></div>
