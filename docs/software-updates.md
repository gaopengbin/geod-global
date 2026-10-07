# 软件更新、通知和安装体积

当前开发代码已接入更新客户端和软件内通知。正式更新与公告渠道尚未上线：本轮按既有要求不制作安装包、不覆盖安装版、不发布 Release，也不向真实用户发送公告。已发行的 rc.3 没有更新客户端，需要首次手动安装含本模块的签名版本，此后才能在软件内更新。

## 软件内流程

- 顶部铃铛打开通知中心：全部 / 未读、单条已读、全部已读、产品公告及版本提醒。已读按公告修订保存，重要公告一次提醒；关闭通知后停止自动拉取及重要提示，手动查看仍可用。当前无需新增软件账号。
- 设置中的“软件更新”显示版本、检查时间、更新内容、下载进度、取消、验证完成和安装确认。只自动检查，不自动下载或安装。后台检查尊重本机代理、窗口可见性和已保存的检查时间。
- 使用真实 Tauri updater，要求内容签名及签名绑定的版本。安装前重新核对缓存 SHA-256 和原始签名。缓存可在重新检查同一版本后恢复，清单变更不会继续使用旧签名。
- 安装仅在发行版启用。先冻结新操作、等待当前原生命令结束，再检查任务是否仍有下载、处理或输出收尾，以及 Agent 是否仍忙。活动任务、未知 Agent 状态或正在退出都会阻止安装。
- Global 使用独立产品 ID、公开配置、密钥和频道，不读取 geod-agent 的账号、通知已读记录或发行私钥。模型没有更新、安装或公告发布工具。
- 通知是经过内容签名的公开 JSON。原生验证产品、数量、日期、版本及动作类型；远端内容不执行 HTML、JavaScript、任意链接或原生命令。正常内容按明文排版。拉取失败保留已校验的缓存。
- 通知状态损坏时保留原文件并恢复默认，避免可选消息模块阻止主软件启动。核心工程、文件及凭据不受该恢复影响。

参考为 geod-agent 当前 `desktop_settings.rs`、`message-center.tsx` 及其更新 / 消息验收记录；Global 代码与数据目录保持独立。更新库固定为 2.13.1，配套 Tauri 2.12.1 / CLI 2.12.0。[官方更新机制](https://v2.tauri.app/plugin/updater/)说明内容签名和分离下载 / 安装方式。更新签名不等同于 Windows Authenticode 证书。

## 独立发行配置

构建环境的 `GEOD_GLOBAL_DISTRIBUTION` 为公开 JSON：

```json
{
  "product": "xyz.laogao.geod.global",
  "endpoint": "https://YOUR-GLOBAL-CHANNEL/windows-x86_64/latest.json",
  "pubkey": "GLOBAL_PUBLIC_KEY_CONTENT",
  "messagesEndpoint": "https://YOUR-GLOBAL-CHANNEL/notifications-signed.json"
}
```

这些是格式示例，不是已上线地址。正式构建只接受冻结的公开配置和 HTTPS。没有配置时，显示渠道待启用，不能显示“已是最新版”。只有 Debug 可通过 `GEOD_GLOBAL_DEV_DISTRIBUTION` 指向隔离配置文件使用 127.0.0.1 / IPv6 loopback HTTP。测试私钥不会成为发行身份。

`scripts/distribution-channel.py prepare-build --channel <公开配置.json> --output <公开构建环境文件>` 校验并产生构建变量。应在编译前设置此公开变量。版本、来源及安装包仍按现有干净提交发行流程检查。

安装包完成后，独立签名步骤使用专用 `GEOD_GLOBAL_SIGNING_PRIVATE_KEY` 和 `GEOD_GLOBAL_SIGNING_PASSWORD`。打包脚本会移除编译子进程中的签名私钥变量；签名步骤不重编译，私钥仅传给官方签名器，输出不记录私钥或签名器诊断。不要复用 geod-agent 的身份。

```text
python scripts/distribution-channel.py sign-update --installer <安装包.exe> --version <版本> --url <实际HTTPS下载地址> --notes <更新说明.txt> --output <latest.json>
python scripts/distribution-channel.py sign-notifications --source distribution/notifications.json --output <notifications-signed.json>
```

更新 JSON 增加 Global 产品标识，签名含版本，平台为 `windows-x86_64`。通知源包含中英文标题和正文、修订、发布日期 / 过期日、适用版本；动作只允许 `updates` 或 `sources`。默认源为空，不编造公告。服务器应先上传并验证实际签名文件与内容，再原子替换最新清单，保留旧版本以供人工恢复。上述脚本不上传、安装或公开发布。

## 当前体积和精简

实际已发布 rc.3：安装包 **69,489,064 字节（69.49 MB）**，便携 ZIP **78,819,870 字节（78.82 MB）**。这两个文件属于 Agent 接入之前的版本，不能当作当前开发版完整安装包体积。

当前本地 Agent 运行环境约 **417.76 MB 未压缩**：Codex 324.70 MB、Node 91.38 MB、Agent 适配代码 1.44 MB、许可约 0.23 MB。适配代码从 2,604,712 字节降到 1,441,194 字节，约减小 44.7%；保留函数名、完整许可与实际 stdio 检查。

没有附带 Java/JRE、本地 OCR、Python/GDAL 环境、整套 npm 依赖或 Codex 的 shell / voice / code-mode 辅助宿主。GIS 读取与处理、Office 文本解析是现有原生 Rust 代码；不另打入 geod-agent 截图中的整套外部环境。二维前端没有 Cesium 资源。PDF.js 必要资源约 3.53 MB，行政区及底图约 61.68 MB 原始文件，会被 Tauri 压缩嵌入；不能简单删除全球边界来换取“精简”。

打包遗漏也已修正：发行脚本按已校验清单携带 `agent-runtime`，固定来源、大小、摘要、构建回执和许可，排除额外文件。以前的脚本没有携带这个新增环境。增加这些正在使用的文件会使下一完整安装包变大；本轮没有生成新安装包，不能把未压缩大小、Debug exe 或估算压缩比写成实际安装包大小。Codex 和 Node 承担当前自然语言 Agent，直接删除会失去功能。若后续需要更小的首次下载，可另做受签名约束的 Agent 按需安装，但本轮没有将其冒充已完成。

完整机器记录见 [体积核对](../prototype/qa/distribution-size-audit.json)。可重复运行 `python scripts/audit-distribution-size.py --release v0.1.0-rc.3`，不生成安装包。

## 验证范围

- 实际 updater SDK + 独立 loopback HTTP：正确内容签名、真实分片下载、篡改、错误公钥、清单与签名版本不符、缺少签名版本、截断、相同 / 旧版本不下载。
- 原生消息：实际签名验证与篡改拒绝、产品 / 动作 / 版本校验、时效过滤、已读和修订、持久化重开、坏状态保留；原生维护门禁覆盖并发命令阻止及失败释放。
- 更新、设置和首页组件交互，开发版安装禁用、安装确认、离线通知缓存，以及明暗外观和桌面 / 窄屏实看。
- 压缩后 Node / Codex 实际 stdio 启动与快照、运行环境许可和打包 / 发行测试。

没有执行安装器。真实安装升级、升级后重启 / 回退、首个正式签名更新源和正式公告发布仍须在发行时单独验收。
