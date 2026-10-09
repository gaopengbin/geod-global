# GeoD Global for Windows

Windows 10 / 11 x64 桌面版。此包为 **0.2.0-rc.2 发布候选**，供试用和反馈，
尚非稳定正式版。文件未签名，Windows 可能显示未知发布者提示。
本包的功能范围和限制见 [RELEASE-NOTES.md](RELEASE-NOTES.md)。

## 安装与启动

1. 安装 [Microsoft Edge WebView2 Evergreen Runtime](https://developer.microsoft.com/microsoft-edge/webview2/)。
   大多数 Windows 设备已包含它；安装程序会检查，但不会下载或安装前置组件。
2. 运行安装程序，或把便携 ZIP **完整解压**到可写目录。
3. 打开 `geod-global-desktop.exe`。不需要 Node.js、Rust、Python、开发服务器或系统 GDAL。

安装程序仅为当前用户安装，默认位置是 `%LOCALAPPDATA%\Programs\GeoD Global`，
并创建开始菜单快捷方式和卸载入口。升级前请在托盘菜单中选择“退出”，再运行新安装程序。

关闭窗口会保留托盘进程，下载和处理继续运行。点击托盘图标可恢复窗口；
托盘菜单的“退出”会停止任务并退出。重新启动后，未完成的文件任务可在“任务”页重试。

## 首次使用

首页可以直接描述所需数据，连接自己的模型后使用 Agent。先预览区域与影像，确认完整任务后下载并执行支持的处理。也可在“探索”选择来源、区域与日期，选择影像并加载预览。下载时可新建并命名工程，
也可追加到当前工程；在“我的数据”查看文件、原始像元和兼容网格的处理结果。
文件卡片的图标按钮提供工作区、所属工程和来源详情入口，悬停可查看操作名称。

“设置”提供界面语言、明暗主题、源文件下载代理及数据源授权。
公开数据无需账号。NASA Earthdata / Copernicus 授权入口已提供，
但这些来源的受保护原文件尚待真实账号验收，候选版暂不开放其下载。
完整范围见 [数据源状态](docs/provider-integration-status.md)；渲染影像不等同于原始科学数据。

Global 邮箱、Google、GitHub 登录通过系统浏览器完成，游客仍可使用本地功能。Global 账号不替代模型连接或 NASA / Copernicus 数据授权，也不提供云同步。

## 文件与备份

便携包只让**应用程序文件**可移动，用户数据仍使用独立应用标识 `xyz.laogao.geod.global`。
工程、任务、原文件、派生成果及持久缩略图位于
`%LOCALAPPDATA%\xyz.laogao.geod.global\runtime`，语言和界面偏好位于应用的 WebView 数据目录。
备份时保留整个应用数据目录；不要同时让多个程序访问同一运行时目录。
账号令牌由 Windows 凭据管理器保存，复制应用目录不会迁移账号授权。

卸载仅移除包内应用文件、快捷方式和卸载入口，保留下载数据、任务、偏好及安装目录中的额外文件。
删除便携包目录也不会删除应用数据。

## CLI、Agent 和校验

`geod-runtime.exe --help` 输出 JSON 命令列表。CLI 必须指定独立 `--data-dir`，
或连接已运行的 loopback `--server`；桌面程序运行时请使用另一个数据目录。
[操作示例](docs/workflows/clip-sentinel-scl.md)、[运行时说明](docs/runtime.md)、
[MCP 说明](docs/mcp.md)、`examples/` 和 `schemas/` 随包提供。
操作示例中的源码构建步骤可跳过，直接使用本包的 `geod-runtime.exe`。

`SHA256SUMS.txt` 校验安装程序和 ZIP；`release-manifest.json` 记录版本、源码提交、
构建参数及包内每个文件的校验值。校验值不能替代数字签名。
`THIRD-PARTY/` 保留依赖许可和适用的源码档案；第一方权利说明见 `FIRST-PARTY-NOTICE.txt`。
本候选包含更新和通知界面，但正式签名更新与公告渠道尚未启用，仍需手动下载升级。出现问题时，可从“设置 → 本地诊断”生成不含敏感数据的报告，
附上版本、复现步骤及错误参考号向发布者反馈；诊断不会自动上传。

## English quick start

This is an unsigned Windows x64 release candidate for evaluation and feedback.
Install the WebView2 Evergreen Runtime, then use the per-user installer or extract
the entire portable ZIP. Run `geod-global-desktop.exe`; development tools and a
companion server are unnecessary. Closing the window keeps tasks running in the
system tray. Use **Quit** in the tray before upgrading or to stop the application.

Public-source workflows are available within the documented limits. Earthdata
and Copernicus account setup is included; protected original downloads remain
unavailable pending real-account verification. Uninstall retains user data.
See the release notes and provider documentation for support boundaries.
