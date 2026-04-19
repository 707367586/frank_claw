//! LLM router — selects provider based on model name prefix.

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;

use async_trait::async_trait;
use clawx_types::error::{ClawxError, Result};
use clawx_types::llm::*;
use clawx_types::traits::LlmProvider;

/// Routes completion requests to the appropriate provider based on model name.
///
/// Model prefix mapping:
/// - `claude-` -> `"anthropic"`
/// - `gpt-` -> `"openai"`
/// - anything else -> `"stub"` (fallback)
#[derive(Clone)]
pub struct LlmRouter {
    providers: HashMap<String, Arc<dyn LlmProvider>>,
    fallback_key: String,
}

impl std::fmt::Debug for LlmRouter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LlmRouter")
            .field("providers", &self.providers.keys().collect::<Vec<_>>())
            .field("fallback_key", &self.fallback_key)
            .finish()
    }
}

impl LlmRouter {
    /// Create a new router with the given named providers.
    /// `fallback_key` is the key used when no prefix matches.
    pub fn new(
        providers: HashMap<String, Arc<dyn LlmProvider>>,
        fallback_key: String,
    ) -> Self {
        Self {
            providers,
            fallback_key,
        }
    }

    /// Return the keys of currently registered providers.
    pub fn provider_keys(&self) -> impl Iterator<Item = &str> {
        self.providers.keys().map(|s| s.as_str())
    }

    /// Determine which provider key to use for a given model name.
    pub fn resolve_key(&self, model: &str) -> &str {
        if model.starts_with("claude-") {
            "anthropic"
        } else if model.starts_with("gpt-") {
            "openai"
        } else if model.starts_with("glm-") {
            "zhipu"
        } else {
            &self.fallback_key
        }
    }

    /// Get the provider for a model, or error if not registered.
    fn provider_for(&self, model: &str) -> Result<&Arc<dyn LlmProvider>> {
        let key = self.resolve_key(model);
        self.providers.get(key).ok_or_else(|| {
            ClawxError::LlmProvider(format!(
                "no provider registered for key '{}' (model: {})",
                key, model
            ))
        })
    }
}

#[async_trait]
impl LlmProvider for LlmRouter {
    async fn complete(&self, request: CompletionRequest) -> Result<LlmResponse> {
        let provider = self.provider_for(&request.model)?;
        provider.complete(request).await
    }

    async fn stream(
        &self,
        request: CompletionRequest,
    ) -> Result<Pin<Box<dyn futures::Stream<Item = Result<LlmStreamChunk>> + Send>>> {
        let provider = self.provider_for(&request.model)?;
        provider.stream(request).await
    }

    async fn test_connection(&self) -> Result<()> {
        // Test all registered providers
        for (key, provider) in &self.providers {
            provider.test_connection().await.map_err(|e| {
                ClawxError::LlmProvider(format!("provider '{}' connection test failed: {}", key, e))
            })?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::StubLlmProvider;

    fn make_router() -> LlmRouter {
        let mut providers: HashMap<String, Arc<dyn LlmProvider>> = HashMap::new();
        providers.insert("stub".to_string(), Arc::new(StubLlmProvider));
        providers.insert("anthropic".to_string(), Arc::new(StubLlmProvider));
        providers.insert("openai".to_string(), Arc::new(StubLlmProvider));
        providers.insert("zhipu".to_string(), Arc::new(StubLlmProvider));
        LlmRouter::new(providers, "stub".to_string())
    }

    fn make_request(model: &str) -> CompletionRequest {
        CompletionRequest {
            model: model.to_string(),
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

    #[test]
    fn routes_claude_models_to_anthropic() {
        let router = make_router();
        assert_eq!(router.resolve_key("claude-3-opus-20240229"), "anthropic");
        assert_eq!(router.resolve_key("claude-3-haiku-20240307"), "anthropic");
        assert_eq!(router.resolve_key("claude-3-5-sonnet-20241022"), "anthropic");
    }

    #[test]
    fn routes_gpt_models_to_openai() {
        let router = make_router();
        assert_eq!(router.resolve_key("gpt-4o"), "openai");
        assert_eq!(router.resolve_key("gpt-4-turbo"), "openai");
        assert_eq!(router.resolve_key("gpt-3.5-turbo"), "openai");
    }

    #[test]
    fn routes_glm_models_to_zhipu() {
        let router = make_router();
        assert_eq!(router.resolve_key("glm-4"), "zhipu");
        assert_eq!(router.resolve_key("glm-4-flash"), "zhipu");
        assert_eq!(router.resolve_key("glm-4-plus"), "zhipu");
        assert_eq!(router.resolve_key("glm-4v"), "zhipu");
    }

    #[test]
    fn routes_unknown_models_to_fallback() {
        let router = make_router();
        assert_eq!(router.resolve_key("llama-3"), "stub");
        assert_eq!(router.resolve_key("mixtral-8x7b"), "stub");
        assert_eq!(router.resolve_key("custom-model"), "stub");
    }

    #[tokio::test]
    async fn complete_routes_to_correct_provider() {
        let router = make_router();

        // All use StubLlmProvider under the hood, so they should all succeed
        let resp = router.complete(make_request("claude-3-opus")).await.unwrap();
        assert!(resp.content.contains("[stub]"));

        let resp = router.complete(make_request("gpt-4o")).await.unwrap();
        assert!(resp.content.contains("[stub]"));

        let resp = router.complete(make_request("llama-3")).await.unwrap();
        assert!(resp.content.contains("[stub]"));
    }

    #[tokio::test]
    async fn stream_routes_to_correct_provider() {
        use futures::StreamExt;

        let router = make_router();
        let mut stream = router.stream(make_request("claude-3-haiku")).await.unwrap();
        let chunk = stream.next().await.unwrap().unwrap();
        assert!(chunk.delta.contains("[stub]"));
    }

    #[tokio::test]
    async fn test_connection_checks_all_providers() {
        let router = make_router();
        router.test_connection().await.unwrap();
    }

    #[tokio::test]
    async fn missing_provider_returns_error() {
        let providers: HashMap<String, Arc<dyn LlmProvider>> = HashMap::new();
        let router = LlmRouter::new(providers, "stub".to_string());

        let result = router.complete(make_request("claude-3-opus")).await;
        assert!(matches!(result, Err(ClawxError::LlmProvider(_))));
    }

    #[test]
    fn router_is_clone() {
        let router = make_router();
        let _clone = router.clone();
    }

    #[test]
    fn router_is_debug() {
        let router = make_router();
        let debug = format!("{:?}", router);
        assert!(debug.contains("LlmRouter"));
    }
}
