use std::collections::BTreeMap;
use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::time::Instant;

use crate::function_tool::FunctionCallError;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::context::boxed_tool_output;
use crate::tools::handlers::parse_arguments;
use crate::tools::registry::CoreToolRuntime;
use crate::tools::registry::ToolExecutor;
use codex_tools::JsonSchema;
use codex_tools::ToolName;
use codex_tools::ToolSpec;

use serde::Deserialize;

#[derive(Deserialize)]
struct SubagentArgs {
    prompt: String,
    #[serde(default)]
    owned_files: Option<String>,
}

fn build_subagent_spec(tool_name: &str, description: &str) -> ToolSpec {
    let properties = BTreeMap::from([
        (
            "prompt".to_string(),
            JsonSchema::string(Some("Task description for the subagent.".to_string())),
        ),
        (
            "owned_files".to_string(),
            JsonSchema::string(Some("Comma-separated focus files (advisory).".to_string())),
        ),
    ]);
    let required = vec!["prompt".to_string()];

    ToolSpec::Function(codex_tools::ResponsesApiTool {
        name: tool_name.to_string(),
        description: description.to_string(),
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::object(properties, Some(required), Some(false.into())),
        output_schema: None,
    })
}

fn build_prompt(tool_name: &str, prompt: &str, owned_files: &Option<String>) -> String {
    let mut lines: Vec<String> = Vec::new();
    match tool_name {
        "build" => {
            lines.push("You are a backend builder. Implement the following task.".into());
            if let Some(files) = owned_files {
                lines.push(format!("Focus area: {files}."));
            }
            lines.push("After changes, verify with relevant commands.".into());
        }
        "build_frontend" => {
            lines.push("You are a frontend builder. Quick task — implement it efficiently.".into());
            if let Some(files) = owned_files {
                lines.push(format!("Focus area: {files}."));
            }
            lines.push("Use existing patterns. Make minimal, clean changes.".into());
        }
        "build_frontend_advanced" => {
            lines.push("You are a senior frontend specialist. Complex task — do it right.".into());
            if let Some(files) = owned_files {
                lines.push(format!("Focus area: {files}."));
            }
            lines.push("Focus on UI/UX quality, design system, responsive, accessibility.".into());
        }
        _ => {}
    }
    lines.push(format!("Task: {prompt}"));
    lines.push("Return: (1) what you did, (2) every file touched, (3) success/failure.".into());
    lines.join("\n")
}

fn model_for_tool(tool_name: &str) -> String {
    match tool_name {
        "build" => std::env::var("CODEX_SWARM_BUILD_MODEL")
            .unwrap_or_else(|_| "opencode-go/deepseek-v4-pro".into()),
        "build_frontend" => std::env::var("CODEX_SWARM_FRONTEND_MODEL")
            .unwrap_or_else(|_| "opencode-go/deepseek-v4-flash".into()),
        "build_frontend_advanced" => std::env::var("CODEX_SWARM_FRONTEND_ADVANCED_MODEL")
            .unwrap_or_else(|_| "composer-2.5-fast".into()),
        _ => "opencode-go/deepseek-v4-flash".into(),
    }
}

fn backend_for_tool(tool_name: &str) -> (&str, &[&str]) {
    match tool_name {
        "build_frontend_advanced" => {
            ("cursor", &["agent", "--print", "--model", "--yolo", "--trust", "--output-format", "json"] as &[&str])
        }
        _ => {
            ("opencode", &["run", "--model", "--format", "json", "--dangerously-skip-permissions"] as &[&str])
        }
    }
}

struct SpawnResult {
    text: String,
    session_id: Option<String>,
}

async fn spawn_and_collect(
    binary: &str,
    base_args: &[&str],
    model: &str,
    prompt: &str,
    existing_session: Option<&str>,
) -> Result<SpawnResult, FunctionCallError> {
    let mut cmd = tokio::process::Command::new(binary);
    for arg in base_args {
        if *arg == "--model" {
            cmd.arg("--model");
            cmd.arg(model);
        } else {
            cmd.arg(*arg);
        }
    }
    if let Some(sid) = existing_session {
        if binary == "opencode" {
            cmd.arg("--session");
            cmd.arg(sid);
            cmd.arg("--fork");
        } else {
            cmd.arg("--resume");
            cmd.arg(sid);
        }
    }
    cmd.arg(prompt);
    cmd.kill_on_drop(true);

    let output = cmd
        .output()
        .await
        .map_err(|e| FunctionCallError::Fatal(format!("Failed to spawn {binary}: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(FunctionCallError::RespondToModel(format!(
            "{binary} exited with {}: {}",
            output.status,
            stderr.chars().take(500).collect::<String>()
        )));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    if binary == "opencode" {
        let mut text = String::new();
        let mut session_id = None;

        for line in stdout.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if let Ok(event) = serde_json::from_str::<serde_json::Value>(line) {
                if session_id.is_none() {
                    session_id = event
                        .get("sessionID")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                }
                if event.get("type").and_then(|v| v.as_str()) == Some("text") {
                    if let Some(t) = event
                        .get("part")
                        .and_then(|p| p.get("text"))
                        .and_then(|v| v.as_str())
                    {
                        text.push_str(t);
                    }
                }
            }
        }

        if text.is_empty() {
            return Ok(SpawnResult {
                text: stdout.chars().take(4000).collect(),
                session_id: None,
            });
        }
        Ok(SpawnResult { text, session_id })
    } else {
        let (text, chat_id) = if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&stdout) {
            let t = parsed
                .get("text")
                .or_else(|| parsed.get("content"))
                .or_else(|| parsed.get("message"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| stdout.chars().take(4000).collect());
            let cid = parsed
                .get("chatId")
                .or_else(|| parsed.get("chat_id"))
                .or_else(|| parsed.get("sessionId"))
                .or_else(|| parsed.get("id"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            (t, cid)
        } else {
            (stdout.chars().take(4000).collect(), None)
        };
        Ok(SpawnResult {
            text,
            session_id: chat_id,
        })
    }
}

async fn query_opencode_cost(session_id: &str) -> Option<String> {
    let sql = format!(
        "select cost, tokens_input, tokens_output, model from session where id = '{}' limit 1",
        session_id.replace('\'', "''")
    );
    let output = tokio::process::Command::new("opencode")
        .args(["db", &sql, "--format", "json"])
        .output()
        .await
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let rows: Vec<serde_json::Value> = serde_json::from_str(&stdout).ok()?;
    let row = rows.first()?;

    let mut parts: Vec<String> = Vec::new();

    if let Some(model) = row.get("model").and_then(|v| v.as_str()) {
        if let Ok(m) = serde_json::from_str::<serde_json::Value>(model) {
            if let Some(id) = m.get("id").or_else(|| m.get("modelID")).and_then(|v| v.as_str()) {
                parts.push(id.to_string());
            }
        }
    }

    if let Some(cost) = row.get("cost").and_then(|v| v.as_f64()) {
        parts.push(format!("${:.4}", cost));
    }
    if let Some(input) = row.get("tokens_input").and_then(|v| v.as_i64()) {
        parts.push(format!("{input} in"));
    }
    if let Some(output_tok) = row.get("tokens_output").and_then(|v| v.as_i64()) {
        parts.push(format!("{output_tok} out"));
    }

    if parts.is_empty() {
        None
    } else {
        Some(parts.join(" | "))
    }
}

fn session_store() -> &'static Mutex<HashMap<String, Option<String>>> {
    static STORE: OnceLock<Mutex<HashMap<String, Option<String>>>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(HashMap::new()))
}

macro_rules! subagent_handler {
    ($name:ident, $tool_name:literal, $description:literal) => {
        pub struct $name;

        #[async_trait::async_trait]
        impl ToolExecutor<ToolInvocation> for $name {
            fn tool_name(&self) -> ToolName {
                ToolName::plain($tool_name)
            }

            fn spec(&self) -> ToolSpec {
                build_subagent_spec($tool_name, $description)
            }

            fn supports_parallel_tool_calls(&self) -> bool {
                true
            }

            async fn handle(
                &self,
                invocation: ToolInvocation,
            ) -> Result<Box<dyn crate::tools::context::ToolOutput>, FunctionCallError> {
                let ToolInvocation { payload, .. } = invocation;
                let ToolPayload::Function { arguments } = payload else {
                    return Err(FunctionCallError::RespondToModel(
                        "subagent handler received unsupported payload".to_string(),
                    ));
                };

                let args: SubagentArgs = parse_arguments(&arguments)?;
                let prompt = build_prompt($tool_name, &args.prompt, &args.owned_files);
                let model = model_for_tool($tool_name);
                let (binary, base_args) = backend_for_tool($tool_name);

                let existing = session_store()
                    .lock()
                    .unwrap()
                    .get($tool_name)
                    .cloned()
                    .flatten();

                let start = Instant::now();
                let result = spawn_and_collect(
                    binary,
                    base_args,
                    &model,
                    &prompt,
                    existing.as_deref(),
                )
                .await;

                match result {
                    Ok(spawn_result) => {
                        session_store()
                            .lock()
                            .unwrap()
                            .insert($tool_name.to_string(), spawn_result.session_id.clone());

                        let cost = if binary == "opencode" {
                            if let Some(ref sid) = spawn_result.session_id {
                                query_opencode_cost(sid).await
                            } else {
                                None
                            }
                        } else {
                            None
                        };

                        let elapsed = start.elapsed().as_secs_f64();
                        let reused = existing.is_some();

                        let mut header = format!(
                            "[{} | {:.1}s | {}",
                            $tool_name,
                            elapsed,
                            if reused { "reused" } else { "fresh" }
                        );
                        if let Some(ref c) = cost {
                            header.push_str(&format!(" | {c}"));
                        }
                        header.push_str(&format!(" | {model}]\n\n"));

                        let full = format!("{header}{}", spawn_result.text);
                        Ok(boxed_tool_output(FunctionToolOutput::from_text(full, Some(true))))
                    }
                    Err(e) => {
                        session_store()
                            .lock()
                            .unwrap()
                            .insert($tool_name.to_string(), None);
                        Err(e)
                    }
                }
            }
        }

        impl CoreToolRuntime for $name {}
    };
}

subagent_handler!(
    BuildHandler,
    "build",
    "Use for backend implementation: features, bug fixes, refactoring, API, database, logic. Uses DeepSeek Pro (opencode). Do NOT edit backend files yourself. Run concurrently with frontend tools for full-stack work. Session reused across calls. Use owned_files to suggest focus areas (advisory)."
);

subagent_handler!(
    BuildFrontendHandler,
    "build_frontend",
    "Use for SIMPLE frontend work: small UI fixes, CSS tweaks, copy changes, minor component adjustments. Uses DeepSeek Flash (opencode) — fast, cheap. Do NOT edit UI files yourself. For complex frontend, use build_frontend_advanced. Session reused across calls."
);

subagent_handler!(
    BuildFrontendAdvancedHandler,
    "build_frontend_advanced",
    "Use for COMPLEX frontend: full components, page layouts, design-system refactors, animations, responsive overhauls. Uses Cursor Composer 2.5 Fast. Do NOT use for simple tweaks — use build_frontend instead. Session reused across calls."
);
