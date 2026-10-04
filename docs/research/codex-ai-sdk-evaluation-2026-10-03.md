# Codex + AI SDK：Agent 选型调研与独立验证

记录日期：2026-10-03。来源：用户授权的旁聊调研与独立协议样例验证。用户要求将结果记录并告知主会话；本记录不代表要求中断当前数据源接入或立即替换现有运行时。

## 建议与证据等级

建议以 **Codex 为唯一 Agent 编排循环，AI SDK 为模型供应商适配层**。二者之间需要 Responses 协议桥，不能只把 AI SDK provider 配置加进 Codex 就认为接入完成。

本地最小路径已经真实跑通：Codex App Server → 本地 Responses 桥 → AI SDK → 模型网关 → Codex 执行只读工具 → 工具结果回传 → 最终回答。Codex 退出后，同一磁盘会话恢复并查询更新的工具结果也通过。

**证据边界：只有 DeepSeek 配置名称对应的网关路线通过。其他厂商接入、完整推理协议和 GeoD 真实下载尚未通过本验证。** 模型名称是请求配置，未独立审计网关实际使用的上游厂商或型号。

## 公开实现参考

以下是本轮查阅的项目文档与源码；源码存在不等于其所有模型或部署环境均已实际验收。

| 参考 | 查阅内容 | 对 GeoD 的意义 |
|---|---|---|
| [relay-ai Codex 文档](https://github.com/jacob-bd/relay-ai/blob/main/docs/CODEX.md) | Codex 经本地 Responses proxy 调用 AI SDK providers | 与建议架构直接相符 |
| [relay-ai Responses adapter](https://github.com/jacob-bd/relay-ai/blob/main/src/codex-responses-adapter.ts) | 每次代理请求只做模型调用，工具定义不附带执行器；Codex 拥有工具循环 | 避免 Codex 和 SDK 同时循环并重复执行业务工具 |
| [relay-ai adapter tests](https://github.com/jacob-bd/relay-ai/blob/main/tests/codex-responses-adapter.test.ts) | 工具往返、推理/签名、命名空间与自定义工具等转换案例 | 可作为协议验收清单；测试代码不等于 GeoD 已验收 |
| [CodePilot unified adapter](https://github.com/op7418/CodePilot/blob/main/src/lib/codex/proxy/unified-adapter.ts) | 桌面 App Server + Responses proxy + AI SDK；部分主机内建工具有 SDK 内循环与旁路事件 | 有桌面集成参考价值，但不能照搬双循环；复用前需核查当前许可 |
| [CodePilot dynamic tool bridge](https://github.com/op7418/CodePilot/blob/main/src/lib/codex/dynamic-tool-bridge.ts) | 动态工具通过 `item/tool/call` 转发，衔接原生审批 | 可借鉴事件边界与工具调用关联方式 |
| [CodePilot Windows proxy 验证记录](https://github.com/op7418/CodePilot/blob/main/docs/exec-plans/active/windows-codex-loopback-proxy.md) | Windows 本地代理与 Clash 等环境存在专项兼容性问题 | 本地回环、系统代理和安装版运行时须分别验收 |
| [AI SDK Codex harness 文档](https://ai-sdk.dev/providers/ai-sdk-harnesses/codex) | `@ai-sdk/harness-codex` 为实验性 harness，包含沙箱桥与 App Server 支持 | 它是执行器封装，不自动替代多供应商模型协议桥 |
| [AI SDK Providers](https://ai-sdk.dev/providers/ai-sdk-providers) | 原生及兼容 provider 接口 | 模型注册层可以统一，但不能假定兼容接口覆盖全部厂商能力 |

## 推荐的集成边界

```text
软件内对话与模型选择
        ↓
GeoD Agent 会话服务
        ↓
Codex App Server（编排、会话持久化、工具请求）
        ↓ 模型请求
本地 Responses 协议桥
        ↓
AI SDK Provider 注册表 → 选定供应商 / 合法模型网关

Codex 工具请求 → GeoD 工具适配 → 现有工程 / 任务 / 处理核心
```

- GeoD 工程、下载队列、取消与恢复仍由现有业务核心维护；模型调用工具来读写业务状态，不生成另一套任务生命周期。
- 对话、工具调用 ID、业务任务 ID 和工程 ID 分别关联，避免把一段模型回答当作完成依据。
- 多家 AI 的设置结构应包含供应商、接口协议、模型 ID、连接地址、凭据引用和能力声明。模型下拉列表按供应商分组即可；可用性必须以真实请求和工具往返验证。
- 桌面端凭据由主进程及系统安全存储管理，不放到前端状态、日志或业务文件里。用户界面可以提供统一入口，内部按供应商协议适配。
- GeoD Global 必须独立打包自己的固定版本运行时和依赖；不得把本次样例的兄弟仓库绝对路径作为应用运行依赖。复用代码或库前需核查来源、许可、版本和兼容性。

## 本次实测

运行环境：Windows，Codex App Server **0.159.2**，AI SDK **7.0.127**，`@ai-sdk/openai-compatible` **3.0.62**，`@ai-sdk/anthropic` **4.0.71**。请求经本机 SSH 转发访问已有模型网关。临时转发已停止；没有修改网关配置。

| 请求配置 | AI SDK / 网关接口 | 实际结果 | 判定 |
|---|---|---|---|
| `deepseek-v4-flash` | OpenAI compatible Chat Completions | 连续两次工具调用、准确返回随机校验码、Codex 退出重启后恢复会话并读取更新状态 | 最小核心架构通过 |
| `claude-haiku-4-5` | Anthropic Messages | 网关返回 `No available channels for this model` | 未验收，不据此判断 Anthropic 协议桥兼容性 |
| `claude-haiku-4-5` | OpenAI compatible Chat Completions | 同上 | 未验收 |
| `gemini-2.5-flash` | OpenAI compatible Chat Completions | 同上 | 未验收；没有测试 Google 原生协议 |

通过案例的实际步骤：

1. 模型请求 `probe_lookup_scene`，工具返回随机场景 ID。
2. 模型把该 ID 传给 `probe_job_status`，工具返回状态与随机校验码。
3. 检查最终回答是否包含工具实际返回的状态和校验码。
4. 完全退出 Codex 子进程，重新启动后 `thread/resume` 恢复原会话。
5. 改变工具状态与校验码，要求复用历史场景 ID 查询。检查只调用状态工具，并返回新值。

工具是只读模拟数据；模型请求、Codex 编排、工具参数传递、返回值比对和进程重启是真实执行。本次没有影像下载，也不代表软件内 Agent 已实现。

初次 DeepSeek 测试触发了真实工具调用，但后续请求出现接口错误。样例修正了相邻助手消息合并与 SSE 输出项完成顺序，并限制为两个测试工具后，通过上述完整链路。此后未复测原始版本，因此不能把初次所有错误都归因于同一原因。

## 未验收的能力与后续顺序

当前样例忽略推理块，并且只暴露两个测试工具。推理签名/加密内容、图片与附件、自定义工具、命名空间、并行调用、取消、审批、上下文压缩、完整错误恢复及安装版运行时均未验收，不能把样例直接作为生产协议桥。

建议后续按以下顺序实施，保持当前数据源工作安排：

1. 固定 Codex、AI SDK 和协议版本，补齐供应商适配、错误分类及真实可用通道测试。
2. 使用 GeoD 已有 MCP / 业务核心跑通真实只读工具，并以业务状态而非模型文本验收。
3. 接入真实写入工具、权限边界、取消、会话恢复和任务恢复，避免重复执行。
4. 加入软件内对话与供应商分组选择；分别验收开发环境和独立安装环境。

本记录是选型建议和验证证据，不是已批准的生产实现或发布结论。

## 持久化证据位置

已将 8 个源文件和报告复制到持久目录，逐文件 SHA-256 比对一致：

`C:\Users\Administrator\Documents\Codex\research\geod-codex-ai-sdk-20261003`

- `README.md`：验证说明和复验方式。
- `verify.mjs`：独立样例源码；它不属于 GeoD Global 的运行依赖。
- `package.json` / `package-lock.json`：样例依赖及锁定。
- `report.json`：最终汇总，`coreArchitectureProven=true`，`allProviderRoutesPassed=false`。
- `report-three-models.json`：修正后 DeepSeek 通过及其他模型失败的原始记录。
- `report-claude-compatible.json`：Claude 兼容接口复验记录。
- `report-deepseek-initial.json`：首次失败证据。

没有复制凭据、`CODEX_HOME`、会话数据库或 `node_modules`；没有提交或发布代码。后续需在主会话明确产品实现范围后，再将可移植代码纳入正常开发与验收。
