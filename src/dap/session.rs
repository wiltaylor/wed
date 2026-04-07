//! High-level DAP session: wraps a `DapClient` and tracks debuggee state
//! (threads, current frame, scopes, variables, current line).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{anyhow, Result};
use serde_json::{json, Value};

use crate::dap::breakpoints::Breakpoint;
use crate::dap::client::{DapClient, DapResponse};

#[derive(Debug, Clone, Default)]
pub struct DapThread {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Clone, Default)]
pub struct StackFrame {
    pub id: i64,
    pub name: String,
    pub source_path: Option<PathBuf>,
    pub line: u32,
    pub column: u32,
}

#[derive(Debug, Clone, Default)]
pub struct Scope {
    pub name: String,
    pub variables_reference: i64,
    pub expensive: bool,
}

#[derive(Debug, Clone, Default)]
pub struct Variable {
    pub name: String,
    pub value: String,
    pub type_: Option<String>,
    pub variables_reference: i64,
}

/// A debug session: high-level wrapper over a `DapClient`.
pub struct DapSession {
    pub client: Arc<DapClient>,
    pub threads: Vec<DapThread>,
    pub current_thread: Option<i64>,
    pub stack_frames: Vec<StackFrame>,
    pub scopes: Vec<Scope>,
    pub variables: Vec<Variable>,
    pub current_line: Option<(PathBuf, u32)>,
}

impl DapSession {
    pub fn new(client: Arc<DapClient>) -> Self {
        Self {
            client,
            threads: Vec::new(),
            current_thread: None,
            stack_frames: Vec::new(),
            scopes: Vec::new(),
            variables: Vec::new(),
            current_line: None,
        }
    }

    fn check(resp: DapResponse) -> Result<DapResponse> {
        if !resp.success {
            return Err(anyhow!(
                "dap {} failed: {}",
                resp.command,
                resp.message.clone().unwrap_or_default()
            ));
        }
        Ok(resp)
    }

    pub async fn initialize(&self, adapter_id: &str) -> Result<DapResponse> {
        let args = json!({
            "clientID": "wed",
            "clientName": "wed",
            "adapterID": adapter_id,
            "linesStartAt1": true,
            "columnsStartAt1": true,
            "pathFormat": "path",
            "supportsRunInTerminalRequest": false,
        });
        Self::check(self.client.request("initialize", Some(args)).await?)
    }

    pub async fn launch(&self, config: Value) -> Result<DapResponse> {
        Self::check(self.client.request("launch", Some(config)).await?)
    }

    pub async fn attach(&self, config: Value) -> Result<DapResponse> {
        Self::check(self.client.request("attach", Some(config)).await?)
    }

    pub async fn set_breakpoints(
        &self,
        source: &Path,
        breakpoints: &[Breakpoint],
    ) -> Result<DapResponse> {
        let bps: Vec<Value> = breakpoints
            .iter()
            .filter(|b| b.enabled)
            .map(|b| {
                let mut o = serde_json::Map::new();
                o.insert("line".into(), json!(b.line));
                if let Some(c) = &b.condition {
                    o.insert("condition".into(), json!(c));
                }
                if let Some(h) = &b.hit_condition {
                    o.insert("hitCondition".into(), json!(h));
                }
                if let Some(l) = &b.log_message {
                    o.insert("logMessage".into(), json!(l));
                }
                Value::Object(o)
            })
            .collect();
        let args = json!({
            "source": { "path": source.to_string_lossy() },
            "breakpoints": bps,
            "lines": breakpoints.iter().filter(|b| b.enabled).map(|b| b.line).collect::<Vec<_>>(),
            "sourceModified": false,
        });
        Self::check(self.client.request("setBreakpoints", Some(args)).await?)
    }

    pub async fn configuration_done(&self) -> Result<DapResponse> {
        Self::check(self.client.request("configurationDone", None).await?)
    }

    pub async fn continue_(&self, thread_id: i64) -> Result<DapResponse> {
        Self::check(
            self.client
                .request("continue", Some(json!({ "threadId": thread_id })))
                .await?,
        )
    }

    pub async fn next(&self, thread_id: i64) -> Result<DapResponse> {
        Self::check(
            self.client
                .request("next", Some(json!({ "threadId": thread_id })))
                .await?,
        )
    }

    pub async fn step_in(&self, thread_id: i64) -> Result<DapResponse> {
        Self::check(
            self.client
                .request("stepIn", Some(json!({ "threadId": thread_id })))
                .await?,
        )
    }

    pub async fn step_out(&self, thread_id: i64) -> Result<DapResponse> {
        Self::check(
            self.client
                .request("stepOut", Some(json!({ "threadId": thread_id })))
                .await?,
        )
    }

    pub async fn pause(&self, thread_id: i64) -> Result<DapResponse> {
        Self::check(
            self.client
                .request("pause", Some(json!({ "threadId": thread_id })))
                .await?,
        )
    }

    pub async fn terminate(&self) -> Result<DapResponse> {
        Self::check(self.client.request("terminate", None).await?)
    }

    pub async fn evaluate(&self, expr: &str, frame_id: Option<i64>) -> Result<DapResponse> {
        let mut args = serde_json::Map::new();
        args.insert("expression".into(), json!(expr));
        if let Some(f) = frame_id {
            args.insert("frameId".into(), json!(f));
        }
        args.insert("context".into(), json!("repl"));
        Self::check(
            self.client
                .request("evaluate", Some(Value::Object(args)))
                .await?,
        )
    }

    pub async fn threads(&mut self) -> Result<&[DapThread]> {
        let resp = Self::check(self.client.request("threads", None).await?)?;
        self.threads.clear();
        if let Some(arr) = resp
            .body
            .as_ref()
            .and_then(|b| b.get("threads"))
            .and_then(|v| v.as_array())
        {
            for t in arr {
                self.threads.push(DapThread {
                    id: t.get("id").and_then(|v| v.as_i64()).unwrap_or(0),
                    name: t
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                });
            }
        }
        Ok(&self.threads)
    }

    pub async fn stack_trace(&mut self, thread_id: i64) -> Result<&[StackFrame]> {
        let resp = Self::check(
            self.client
                .request(
                    "stackTrace",
                    Some(json!({ "threadId": thread_id, "startFrame": 0, "levels": 64 })),
                )
                .await?,
        )?;
        self.stack_frames.clear();
        if let Some(arr) = resp
            .body
            .as_ref()
            .and_then(|b| b.get("stackFrames"))
            .and_then(|v| v.as_array())
        {
            for f in arr {
                let frame = StackFrame {
                    id: f.get("id").and_then(|v| v.as_i64()).unwrap_or(0),
                    name: f
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    source_path: f
                        .get("source")
                        .and_then(|s| s.get("path"))
                        .and_then(|v| v.as_str())
                        .map(PathBuf::from),
                    line: f.get("line").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
                    column: f.get("column").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
                };
                self.stack_frames.push(frame);
            }
        }
        if let Some(top) = self.stack_frames.first() {
            if let Some(p) = &top.source_path {
                self.current_line = Some((p.clone(), top.line));
            }
        }
        Ok(&self.stack_frames)
    }

    pub async fn scopes(&mut self, frame_id: i64) -> Result<&[Scope]> {
        let resp = Self::check(
            self.client
                .request("scopes", Some(json!({ "frameId": frame_id })))
                .await?,
        )?;
        self.scopes.clear();
        if let Some(arr) = resp
            .body
            .as_ref()
            .and_then(|b| b.get("scopes"))
            .and_then(|v| v.as_array())
        {
            for s in arr {
                self.scopes.push(Scope {
                    name: s
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    variables_reference: s
                        .get("variablesReference")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0),
                    expensive: s
                        .get("expensive")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false),
                });
            }
        }
        Ok(&self.scopes)
    }

    pub async fn variables(&mut self, variables_reference: i64) -> Result<&[Variable]> {
        let resp = Self::check(
            self.client
                .request(
                    "variables",
                    Some(json!({ "variablesReference": variables_reference })),
                )
                .await?,
        )?;
        self.variables.clear();
        if let Some(arr) = resp
            .body
            .as_ref()
            .and_then(|b| b.get("variables"))
            .and_then(|v| v.as_array())
        {
            for v in arr {
                self.variables.push(Variable {
                    name: v
                        .get("name")
                        .and_then(|x| x.as_str())
                        .unwrap_or("")
                        .to_string(),
                    value: v
                        .get("value")
                        .and_then(|x| x.as_str())
                        .unwrap_or("")
                        .to_string(),
                    type_: v.get("type").and_then(|x| x.as_str()).map(String::from),
                    variables_reference: v
                        .get("variablesReference")
                        .and_then(|x| x.as_i64())
                        .unwrap_or(0),
                });
            }
        }
        Ok(&self.variables)
    }
}
