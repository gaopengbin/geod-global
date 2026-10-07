//! Conversation permission is issued by private desktop IPC, never a model tool.
//! Mode metadata is separate from the existing job/plan store; all work still
//! commits through JobManager's hash, source, entitlement and idempotency checks.
use geod_runtime::JobManager;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};
use tokio::sync::Mutex;

#[derive(Clone, Copy, Default, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum ExecutionMode {
    #[default]
    ConfirmEach,
    FullAccess,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Grant {
    binding: String,
    mode: ExecutionMode,
}
#[derive(Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Policy {
    #[serde(default)]
    default_mode: ExecutionMode,
    #[serde(default)]
    conversations: BTreeMap<String, Grant>,
}
struct HumanRequest {
    binding: String,
    id: String,
    retry: bool,
    cancel: bool,
    consumed: BTreeSet<String>,
}
pub struct ExecutionPolicy {
    path: PathBuf,
    state: Mutex<Policy>,
    requests: Mutex<BTreeMap<String, HumanRequest>>,
}
#[derive(Default)]
pub struct TurnScope<'a> {
    pub request_id: Option<&'a str>,
    pub read_only: bool,
}
fn valid_id(id: &str) -> bool {
    id.len() == 36
        && id.bytes().enumerate().all(|(i, b)| {
            if [8, 13, 18, 23].contains(&i) {
                b == b'-'
            } else {
                b.is_ascii_digit() || (b'a'..=b'f').contains(&b)
            }
        })
}
impl ExecutionPolicy {
    pub async fn open(home: PathBuf) -> Result<Self, String> {
        let path = home.join("execution-policy.json");
        let state = match tokio::fs::read(&path).await {
            Ok(bytes) if bytes.len() <= 32_768 => serde_json::from_slice::<Policy>(&bytes)
                .map_err(|_| "Agent execution permissions could not be read.")?,
            Ok(_) => return Err("Agent execution permissions could not be read.".into()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Policy::default(),
            Err(_) => return Err("Agent execution permissions could not be read.".into()),
        };
        if state.conversations.len() > 50
            || state
                .conversations
                .iter()
                .any(|(id, g)| !valid_id(id) || !valid_id(&g.binding))
        {
            return Err("Agent execution permissions could not be read.".into());
        }
        Ok(Self {
            path,
            state: Mutex::new(state),
            requests: Mutex::new(BTreeMap::new()),
        })
    }
    pub async fn mode(&self, session: Option<&str>, binding: &str) -> ExecutionMode {
        let state = self.state.lock().await;
        match session {
            Some(id) => state
                .conversations
                .get(id)
                .filter(|g| g.binding == binding)
                .map(|g| g.mode)
                .unwrap_or_default(),
            None => state.default_mode,
        }
    }
    pub async fn grant(
        &self,
        session: Option<&str>,
        binding: &str,
        mode: ExecutionMode,
    ) -> Result<(), String> {
        if !valid_id(binding) || session.is_some_and(|id| !valid_id(id)) {
            return Err("Invalid Agent execution permission.".into());
        }
        let mut state = self.state.lock().await;
        let mut next = Policy {
            default_mode: state.default_mode,
            conversations: state.conversations.clone(),
        };
        if let Some(id) = session {
            if next.conversations.len() >= 50 && !next.conversations.contains_key(id) {
                return Err("Agent conversation limit reached.".into());
            }
            next.conversations.insert(
                id.into(),
                Grant {
                    binding: binding.into(),
                    mode,
                },
            );
        } else {
            next.default_mode = mode;
        }
        let temporary = self.path.with_extension("json.tmp");
        tokio::fs::write(&temporary, serde_json::to_vec(&next).unwrap())
            .await
            .map_err(|_| "Agent execution permission could not be saved.")?;
        tokio::fs::rename(temporary, &self.path)
            .await
            .map_err(|_| "Agent execution permission could not be saved.")?;
        *state = next;
        Ok(())
    }
    pub async fn public(&self, session: Option<&str>, binding: &str) -> Value {
        json!({"mode":self.mode(session,binding).await,"defaultMode":self.state.lock().await.default_mode,
            "scope":"managed-projects-and-files","modelCanChangePermission":false})
    }
    pub async fn begin_human_request(&self, session: &str, binding: &str, text: &str) -> String {
        let id = super::registry::new_id();
        let text = text.to_lowercase();
        let negated = ["不要", "别", "不需要", "do not", "don't", "never"]
            .iter()
            .any(|word| text.contains(word));
        let request = HumanRequest {
            binding: binding.into(),
            id: id.clone(),
            retry: !negated
                && [
                    "重试",
                    "重新下载",
                    "重新执行",
                    "重新处理",
                    "再试一次",
                    "retry",
                ]
                .iter()
                .any(|word| text.contains(word)),
            cancel: !negated
                && ["取消", "停止下载", "停止任务", "停止处理", "cancel"]
                    .iter()
                    .any(|word| text.contains(word)),
            consumed: BTreeSet::new(),
        };
        self.requests.lock().await.insert(session.into(), request);
        id
    }
    pub async fn call(
        &self,
        manager: JobManager,
        session: &str,
        binding: &str,
        name: &str,
        args: Value,
        scope: TurnScope<'_>,
    ) -> Result<Value, String> {
        let TurnScope {
            request_id,
            read_only,
        } = scope;
        let state = self.state.lock().await;
        let grant = state
            .conversations
            .get(session)
            .filter(|g| g.binding == binding)
            .ok_or("No native execution permission exists for this conversation.")?;
        if name == "geod_execution_policy" {
            if args != json!({}) {
                return Err("Execution permission accepts no arguments.".into());
            }
            return Ok(
                json!({"mode":grant.mode,"canExecutePlans":grant.mode==ExecutionMode::FullAccess&&!read_only,"scope":"managed-projects-and-files","modelCanChangePermission":false}),
            );
        }
        if grant.mode != ExecutionMode::FullAccess && name != "geod_job_control" {
            return Err("This conversation requires confirmation. Ask the user to confirm in chat or enable automatic execution.".into());
        }
        if read_only {
            return Err("A background task needs attention. Wait for a new human request before starting more work.".into());
        }
        // Hold the native permission lock until the commit finishes. Revocation
        // stops subsequent operations; it never cancels an already queued task.
        if name == "geod_plan_execute" {
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase", deny_unknown_fields)]
            struct Request {
                plan_id: String,
                plan_hash: String,
            }
            let request: Request = serde_json::from_value(args)
                .map_err(|_| "Invalid Agent plan execution arguments.")?;
            return manager
                .approve_agent_plan(session, &request.plan_id, &request.plan_hash)
                .await;
        }
        if name == "geod_job_control" {
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase", deny_unknown_fields)]
            struct Request {
                plan_id: String,
                job_id: String,
                action: String,
            }
            let request: Request = serde_json::from_value(args)
                .map_err(|_| "Invalid Agent task control arguments.")?;
            let mut requests = self.requests.lock().await;
            let authority = requests
                .get_mut(session)
                .filter(|a| a.binding == binding && Some(a.id.as_str()) == request_id)
                .ok_or("Task retry or cancellation requires a fresh human request.")?;
            if !(request.action == "retry" && authority.retry
                || request.action == "cancel" && authority.cancel)
            {
                return Err("Ask the human to request retry or cancellation explicitly.".into());
            }
            let action_key = format!("{}:{}", request.action, request.job_id);
            if authority.consumed.contains(&action_key) {
                return Err(
                    "This task action was already requested in this turn. Read its current status."
                        .into(),
                );
            }
            let plan = manager.agent_plan_status(session, &request.plan_id).await?;
            if !plan["jobs"]
                .as_array()
                .is_some_and(|jobs| jobs.iter().any(|job| job["id"] == request.job_id))
            {
                return Err("Choose a task belonging to this conversation's plan.".into());
            }
            let job = match request.action.as_str() {
                "cancel" => manager.cancel(&request.job_id).await?,
                "retry" => manager.retry(&request.job_id).await?,
                _ => return Err("Choose cancel or retry for the task.".into()),
            };
            authority.consumed.insert(action_key);
            let mut result = manager.agent_plan_status(session, &request.plan_id).await?;
            result["action"] = json!(request.action);
            result["jobId"] = json!(job.id);
            result["jobStatus"] = json!(job.status);
            return Ok(result);
        }
        Err("Unknown Agent execution tool.".into())
    }
}
pub fn definitions() -> Vec<Value> {
    vec![
        json!({"name":"geod_execution_policy","description":"Read the actual native permission for this conversation. Models cannot grant or change permission. Automatic mode allows executing validated native plans within managed workspace files; confirmation mode requires human confirmation in chat or a card.","inputSchema":{"type":"object","properties":{},"additionalProperties":false}}),
        json!({"name":"geod_plan_execute","description":"Execute this conversation's actual native plan only with full-access permission, exact planId and planHash. Native checks source, entitlement, grid, expiry and duplicate execution. Return is a saved project or queued tasks, never completed files. Use only for work requested by the human; planning/estimation requests do not authorize starting tasks. Do not poll repeatedly: the native workflow monitor resumes this conversation after tasks settle.","inputSchema":{"type":"object","properties":{"planId":{"type":"string"},"planHash":{"type":"string"}},"required":["planId","planHash"],"additionalProperties":false}}),
        json!({"name":"geod_job_control","description":"Cancel or retry a real task belonging to an actual plan in this conversation, only when the latest human message explicitly asks; native per-message permission checks this in both execution modes. Read geod_plan_status first. Retry retains the native task identity and preflight. Each job/action is allowed once per human message. Cannot delete files, control other conversations, or retry on autonomous completion events.","inputSchema":{"type":"object","properties":{"planId":{"type":"string"},"jobId":{"type":"string"},"action":{"type":"string","enum":["cancel","retry"]}},"required":["planId","jobId","action"],"additionalProperties":false}}),
    ]
}
#[derive(Clone, Copy)]
pub enum ChatControl {
    Mode(ExecutionMode),
    Confirm(bool),
    Resume,
    Pause,
}
pub fn check_no_pending_decisions(snapshot: &Value) -> Result<(), String> {
    if snapshot["selected"]["entries"]
        .as_array()
        .is_some_and(|entries| {
            entries
                .iter()
                .any(|entry| entry["decision"]["status"] == "pending")
        })
    {
        return Err("Answer the pending decision card before confirming the download task.".into());
    }
    Ok(())
}
pub fn check_decision_confirmation(snapshot: &Value, plan: &str) -> Result<(), String> {
    check_no_pending_decisions(snapshot)?;
    let Some(entries) = snapshot["selected"]["entries"].as_array() else {
        return Ok(());
    };
    let first_review = entries.iter().position(|entry| {
        entry["references"].as_array().is_some_and(|refs| {
            refs.iter()
                .any(|value| value["kind"] == "plan" && value["id"] == plan)
        })
    });
    let Some(first_review) = first_review else {
        return Ok(());
    };
    let review = entries.iter().rev().find(|entry| {
        entry["taskContext"].is_object()
            && entry["references"].as_array().is_some_and(|refs| {
                refs.iter()
                    .any(|value| value["kind"] == "plan" && value["id"] == plan)
            })
    });
    for entry in &entries[first_review + 1..] {
        if entry["decision"]["status"] == "answered"
            && !review.is_some_and(|review| {
                review["taskContext"]["choices"]
                    .as_array()
                    .is_some_and(|choices| {
                        choices
                            .iter()
                            .any(|choice| choice["decisionId"] == entry["decision"]["id"])
                    })
            })
        {
            return Err(
                "Choices changed. Prepare or revise the complete task card before confirming."
                    .into(),
            );
        }
    }
    Ok(())
}
pub fn chat_control(text: &str) -> Option<ChatControl> {
    let text = text
        .trim()
        .trim_end_matches(['。', '！', '!', '.'])
        .to_lowercase();
    match text.as_str() {
        "开启自动执行"
        | "启用自动执行"
        | "允许自动执行"
        | "不用问我"
        | "不用询问我"
        | "不需要我确认"
        | "不需要询问我"
        | "不用再问我"
        | "不要再问我，直接执行"
        | "enable automatic execution"
        | "don't ask me, execute automatically" => {
            Some(ChatControl::Mode(ExecutionMode::FullAccess))
        }
        "关闭自动执行" | "切换逐次确认" | "disable automatic execution" => {
            Some(ChatControl::Mode(ExecutionMode::ConfirmEach))
        }
        "确认" | "确认执行" | "确认方案" | "执行方案" | "开始执行" | "执行吧" | "同意执行"
        | "就这么做吧" | "confirm plan" => Some(ChatControl::Confirm(false)),
        "确认全部" | "确认全部方案" | "执行全部方案" | "confirm all plans" => {
            Some(ChatControl::Confirm(true))
        }
        "继续任务" | "继续执行" | "resume task" => Some(ChatControl::Resume),
        "暂停自动推进" | "停止自动推进" | "pause workflow" => Some(ChatControl::Pause),
        _ => None,
    }
}
#[cfg(test)]
#[path = "agent_execution_tests.rs"]
mod tests;
