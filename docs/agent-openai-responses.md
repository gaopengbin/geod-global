# OpenAI 原生 Responses

2026-10-06。软件内新增 `openai-responses` 协议，使用固定版本 `@ai-sdk/openai` 4.0.83。新建 OpenAI 连接默认使用它；已有兼容 Chat Completions 连接保留原协议、凭据和历史，用户也可明确选择兼容协议。不会因服务失败自动切换协议。

Codex 0.159.2 仍是唯一编排循环，AI SDK 只做协议适配。原生请求设置 `store:false`，附带 `reasoning.encrypted_content`，关闭并行工具调用；GeoD 原生计划仍由用户在软件内单独确认。软件只声明已实现的协议能力，保存模型连接不表示所选接口可用。

加密推理从完整输出项中保存并恢复，采用接口、模型、连接和会话隔离。完整密文参与内容绑定，错误范围、缺失记录、篡改或模糊匹配均拒绝恢复；不使用不完整流事件里的临时密文。恢复记录只含封闭字段及哈希，密文不会进入软件的公开会话记录或界面。无状态回复不复用服务端的临时输出项编号。

实现依据固定 SDK 源码，以及 OpenAI 官方的 [Responses 迁移说明](https://developers.openai.com/api/docs/guides/migrate-to-responses)与[输出项定义](https://developers.openai.com/api/reference/resources/responses/subresources/input_items/methods/list)。用户消息、所选图片和必要工具结果仍发送给所选模型连接；密文恢复不意味着本地推理。

已有开发检查覆盖真实 SDK / Codex、重启恢复、手动与受控自动整理、图片保留、实际 GeoD 核心读取及待确认计划保持。上游生成和凭据为受控数据，不能当作真实云端成功调用。原生开发记录见[桌面内部调用](../.verification/agent-image-native-1791272311535/native-acceptance.json)；模型连接弹窗的保存按钮已移至独立底部区域，并完成一次窄窗口可见性复验。

当前网关的 `gpt-5.6-terra` 原生 Responses 实际请求返回 HTTP 500，尚未成功；保留[失败记录](../.verification/agent-native-protocols-1791272117652/acceptance.json)。兼容协议的既有成功记录仍只证明兼容路线。没有在外部状态不变时重复探测，也没有修改网关路由或用户凭据。

[UTF-8 文本附件](agent-documents.md)已接入；PDF 等二进制附件仍待实现。需要账号的数据源、原生模型云端成功调用和发布环境检查仍保留各自限制，见[Agent 接入状态](agent-integration-status.md)。
