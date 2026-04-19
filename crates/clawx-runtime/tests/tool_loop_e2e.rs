use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use clawx_runtime::tool_loop::{run_with_tools, ToolLoopConfig};
use clawx_tools::approval::AutoApprovalGate;
use clawx_tools::fs::FsMkdirTool;
use clawx_tools::{ToolExecCtx, ToolRegistry};
use clawx_types::error::Result;
use clawx_types::ids::AgentId;
use clawx_types::llm::*;
use clawx_types::traits::LlmProvider;
use futures::Stream;
use std::pin::Pin;

struct Scripted(Mutex<Vec<LlmResponse>>);
#[async_trait]
impl LlmProvider for Scripted {
    async fn complete(&self, _: CompletionRequest) -> Result<LlmResponse> {
        Ok(self.0.lock().unwrap().remove(0))
    }
    async fn stream(
        &self,
        _: CompletionRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<LlmStreamChunk>> + Send>>> {
        unimplemented!()
    }
    async fn test_connection(&self) -> Result<()> {
        Ok(())
    }
}

#[tokio::test]
async fn agent_creates_folder_via_tool_use() {
    let dir = tempfile::tempdir().unwrap();
    let workspace = dir.path().to_path_buf();

    let mut reg = ToolRegistry::new();
    reg.register(Arc::new(FsMkdirTool));

    let llm = Arc::new(Scripted(Mutex::new(vec![
        LlmResponse {
            content: String::new(),
            stop_reason: StopReason::ToolUse,
            tool_calls: vec![ToolCall {
                id: "c1".into(),
                name: "fs_mkdir".into(),
                arguments: serde_json::json!({"path":"claw-demo"}),
            }],
            usage: TokenUsage::default(),
            metadata: None,
        },
        LlmResponse {
            content: "Done — created claw-demo/.".into(),
            stop_reason: StopReason::EndTurn,
            tool_calls: vec![],
            usage: TokenUsage::default(),
            metadata: None,
        },
    ])));

    let exec = ToolExecCtx {
        agent_id: AgentId::new(),
        workspace: workspace.clone(),
        approval: Arc::new(AutoApprovalGate),
    };

    let out = run_with_tools(
        llm,
        Arc::new(reg),
        exec.clone(),
        &exec.agent_id,
        vec![Message {
            role: MessageRole::User,
            content: "Please create a folder named claw-demo".into(),
            blocks: vec![],
            tool_call_id: None,
        }],
        "stub".into(),
        ToolLoopConfig {
            max_iterations: 5,
            max_tokens: 256,
        },
    )
    .await
    .unwrap();

    assert!(
        workspace.join("claw-demo").is_dir(),
        "directory must exist after tool_loop"
    );
    assert!(out.final_content.contains("Done"));
    assert_eq!(out.tool_calls_made, 1);
}
