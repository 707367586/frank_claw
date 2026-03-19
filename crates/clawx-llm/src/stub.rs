//! Stub LLM provider that returns canned responses.

use std::pin::Pin;

use async_trait::async_trait;
use clawx_types::error::Result;
use clawx_types::llm::*;
use clawx_types::traits::LlmProvider;
use futures::stream;

/// A stub LLM provider that returns a fixed response.
/// Used for skeleton testing before real providers are implemented.
#[derive(Debug, Clone)]
pub struct StubLlmProvider;

#[async_trait]
impl LlmProvider for StubLlmProvider {
    async fn complete(&self, request: CompletionRequest) -> Result<LlmResponse> {
        let reply = format!(
            "[stub] Received {} message(s) for model {}",
            request.messages.len(),
            request.model
        );
        Ok(LlmResponse {
            content: reply,
            stop_reason: StopReason::EndTurn,
            tool_calls: vec![],
            usage: TokenUsage::default(),
            metadata: None,
        })
    }

    async fn stream(
        &self,
        request: CompletionRequest,
    ) -> Result<Pin<Box<dyn futures::Stream<Item = Result<LlmStreamChunk>> + Send>>> {
        let reply = format!(
            "[stub] Received {} message(s) for model {}",
            request.messages.len(),
            request.model
        );
        let chunk = LlmStreamChunk {
            delta: reply,
            stop_reason: Some(StopReason::EndTurn),
            usage: Some(TokenUsage::default()),
        };
        Ok(Box::pin(stream::once(async move { Ok(chunk) })))
    }

    async fn test_connection(&self) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::StreamExt;
    use std::sync::Arc;

    fn make_request() -> CompletionRequest {
        CompletionRequest {
            model: "stub-model".to_string(),
            messages: vec![Message {
                role: MessageRole::User,
                content: "Hello".to_string(),
                tool_call_id: None,
            }],
            tools: None,
            temperature: None,
            max_tokens: Some(100),
            stream: false,
        }
    }

    #[tokio::test]
    async fn stub_complete_returns_response() {
        let provider = StubLlmProvider;
        let resp = provider.complete(make_request()).await.unwrap();
        assert!(resp.content.contains("[stub]"));
        assert_eq!(resp.stop_reason, StopReason::EndTurn);
        assert!(resp.tool_calls.is_empty());
    }

    #[tokio::test]
    async fn stub_stream_returns_single_chunk() {
        let provider = StubLlmProvider;
        let mut stream = provider.stream(make_request()).await.unwrap();
        let chunk = stream.next().await.unwrap().unwrap();
        assert!(chunk.delta.contains("[stub]"));
        assert_eq!(chunk.stop_reason, Some(StopReason::EndTurn));
        assert!(stream.next().await.is_none());
    }

    #[tokio::test]
    async fn stub_test_connection_succeeds() {
        StubLlmProvider.test_connection().await.unwrap();
    }

    #[test]
    fn llm_provider_is_object_safe() {
        fn _assert(_: Arc<dyn LlmProvider>) {}
    }
}
