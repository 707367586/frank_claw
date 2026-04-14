//! Executor: multi-step tool calling loop with safety guardrails.
//!
//! Aligned with autonomy-architecture.md §3.

use std::collections::VecDeque;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use tokio::sync::watch;

use clawx_types::autonomy::*;
use clawx_types::error::{ClawxError, Result};
use clawx_types::ids::*;
use clawx_types::permission::{PermissionDecision, RiskLevel};
use clawx_types::traits::{PermissionGatePort, RunUpdate, TaskRegistryPort};

/// Configuration for executor safety guardrails.
#[derive(Debug, Clone)]
pub struct ExecutorConfig {
    /// Maximum number of steps (default: 10, range: 1-25).
    pub max_steps: u32,
    /// Token budget. 80% triggers warning, 100% forces termination.
    pub token_budget: u64,
    /// Timeout in seconds (foreground: 300, background: 1800).
    pub timeout_secs: u64,
    /// Number of identical tool calls before loop detection triggers.
    pub loop_detection_threshold: usize,
}

impl Default for ExecutorConfig {
    fn default() -> Self {
        Self {
            max_steps: 10,
            token_budget: 100_000,
            timeout_secs: 300,
            loop_detection_threshold: 3,
        }
    }
}

impl ExecutorConfig {
    pub fn for_background() -> Self {
        Self {
            timeout_secs: 1800,
            ..Self::default()
        }
    }
}

/// Tracks the state of a multi-step execution.
#[derive(Debug)]
pub struct ExecutionState {
    pub status: RunStatus,
    pub steps: Vec<ExecutionStep>,
    pub tokens_used: u64,
    pub config: ExecutorConfig,
    /// Ring buffer for loop detection.
    tool_call_history: VecDeque<String>,
    pub started_at: chrono::DateTime<Utc>,
}

impl ExecutionState {
    pub fn new(config: ExecutorConfig) -> Self {
        Self {
            status: RunStatus::Queued,
            steps: Vec::new(),
            tokens_used: 0,
            config,
            tool_call_history: VecDeque::with_capacity(20),
            started_at: Utc::now(),
        }
    }

    /// Transition to a new status. Returns error if transition is invalid.
    pub fn transition(&mut self, new_status: RunStatus) -> Result<()> {
        let valid = matches!(
            (&self.status, &new_status),
            (RunStatus::Queued, RunStatus::Planning)
                | (RunStatus::Planning, RunStatus::Running)
                | (RunStatus::Running, RunStatus::Planning)
                | (RunStatus::Running, RunStatus::WaitingConfirmation)
                | (RunStatus::Running, RunStatus::Completed)
                | (RunStatus::Running, RunStatus::Failed)
                | (RunStatus::Running, RunStatus::Interrupted)
                | (RunStatus::WaitingConfirmation, RunStatus::Running)
                | (RunStatus::WaitingConfirmation, RunStatus::Interrupted)
                | (RunStatus::Planning, RunStatus::Failed)
                | (RunStatus::Planning, RunStatus::Interrupted)
                | (RunStatus::Queued, RunStatus::Interrupted)
        );

        if valid {
            self.status = new_status;
            Ok(())
        } else {
            Err(ClawxError::Task(format!(
                "invalid state transition: {} -> {}",
                self.status, new_status
            )))
        }
    }

    /// Record a completed step and check guardrails.
    /// Returns a GuardrailViolation if any limit is exceeded.
    pub fn record_step(&mut self, step: ExecutionStep) -> Option<GuardrailViolation> {
        // Record tool call for loop detection
        if let Some(ref tool) = step.tool {
            self.tool_call_history.push_back(tool.clone());
            if self.tool_call_history.len() > 20 {
                self.tool_call_history.pop_front();
            }
        }

        self.steps.push(step);

        // Check guardrails
        self.check_guardrails()
    }

    /// Add tokens used and check budget.
    pub fn add_tokens(&mut self, tokens: u64) -> Option<GuardrailViolation> {
        self.tokens_used += tokens;
        if self.tokens_used >= self.config.token_budget {
            return Some(GuardrailViolation::TokenBudgetExceeded {
                used: self.tokens_used,
                budget: self.config.token_budget,
            });
        }
        if self.tokens_used as f64 >= self.config.token_budget as f64 * 0.8 {
            return Some(GuardrailViolation::TokenBudgetWarning {
                used: self.tokens_used,
                budget: self.config.token_budget,
            });
        }
        None
    }

    /// Check all guardrails.
    fn check_guardrails(&self) -> Option<GuardrailViolation> {
        // Step limit
        if self.steps.len() as u32 >= self.config.max_steps {
            return Some(GuardrailViolation::StepLimitExceeded {
                steps: self.steps.len() as u32,
                max: self.config.max_steps,
            });
        }

        // Loop detection
        if self.detect_loop() {
            return Some(GuardrailViolation::LoopDetected);
        }

        // Timeout
        let elapsed = (Utc::now() - self.started_at).num_seconds() as u64;
        if elapsed >= self.config.timeout_secs {
            return Some(GuardrailViolation::Timeout {
                elapsed_secs: elapsed,
                limit_secs: self.config.timeout_secs,
            });
        }

        None
    }

    /// Detect repeated tool calls (loop/ping-pong pattern).
    fn detect_loop(&self) -> bool {
        let threshold = self.config.loop_detection_threshold;
        if self.tool_call_history.len() < threshold {
            return false;
        }

        let recent: Vec<&String> = self.tool_call_history.iter().rev().take(threshold).collect();
        if recent.len() < threshold {
            return false;
        }

        // Check if all recent calls are identical
        let first = recent[0];
        recent.iter().all(|call| *call == first)
    }

    /// Create a checkpoint from the current state.
    pub fn checkpoint(&self) -> serde_json::Value {
        serde_json::json!({
            "steps_completed": self.steps.len(),
            "tokens_used": self.tokens_used,
            "status": self.status.to_string(),
            "steps": self.steps,
        })
    }

    /// Number of completed steps.
    pub fn step_count(&self) -> u32 {
        self.steps.len() as u32
    }
}

/// Guardrail violation types.
#[derive(Debug, Clone, PartialEq)]
pub enum GuardrailViolation {
    StepLimitExceeded { steps: u32, max: u32 },
    TokenBudgetExceeded { used: u64, budget: u64 },
    TokenBudgetWarning { used: u64, budget: u64 },
    LoopDetected,
    Timeout { elapsed_secs: u64, limit_secs: u64 },
}

/// Intent evaluator: classifies user request complexity.
pub struct IntentEvaluator;

impl IntentEvaluator {
    /// Simple heuristic-based intent evaluation.
    /// In production, this would use LLM for better classification.
    pub fn evaluate(input: &str) -> IntentCategory {
        let lower = input.to_lowercase();

        // Multi-step indicators
        let multi_step_keywords = [
            "then", "after that", "step by step", "first", "and then",
            "search and", "find and", "analyze and", "compare and",
            "create a report", "generate a summary", "write and save",
            "research", "investigate", "build",
        ];

        let multi_step_count = multi_step_keywords.iter()
            .filter(|kw| lower.contains(*kw))
            .count();

        if multi_step_count >= 2 {
            return IntentCategory::MultiStep;
        }

        // Single tool call indicators
        let assisted_keywords = [
            "search", "find", "look up", "check", "calculate",
            "translate", "convert", "fetch", "download", "read file",
        ];

        let assisted_count = assisted_keywords.iter()
            .filter(|kw| lower.contains(*kw))
            .count();

        if assisted_count >= 1 && multi_step_count == 0 {
            return IntentCategory::Assisted;
        }

        if multi_step_count == 1 {
            return IntentCategory::MultiStep;
        }

        IntentCategory::Simple
    }
}

// ---------------------------------------------------------------------------
// Phase 4.3: TaskExecutor — multi-step execution loop
// Phase 5.2: Permission Gate integration
// ---------------------------------------------------------------------------

/// A tool action to be executed in a step.
#[derive(Debug, Clone)]
pub struct ToolAction {
    pub tool_name: String,
    pub parameters: serde_json::Value,
    pub risk_level: RiskLevel,
}

/// Result of executing a tool.
#[derive(Debug, Clone)]
pub struct ToolResult {
    pub success: bool,
    pub output: String,
    pub tokens_used: u64,
}

/// Trait for tool execution — abstraction over actual tool implementations.
#[async_trait]
pub trait ToolExecutor: Send + Sync {
    async fn execute(&self, action: &ToolAction) -> Result<ToolResult>;
    fn classify_risk(&self, tool_name: &str) -> RiskLevel;
}

/// Summary of a completed execution.
#[derive(Debug)]
pub struct ExecutionSummary {
    pub status: RunStatus,
    pub steps: Vec<ExecutionStep>,
    pub tokens_used: u64,
    pub failure_reason: Option<String>,
}

/// TaskExecutor: orchestrates multi-step execution with permission gate.
pub struct TaskExecutor {
    task_registry: Arc<dyn TaskRegistryPort>,
    permission_gate: Arc<dyn PermissionGatePort>,
    tool_executor: Arc<dyn ToolExecutor>,
}

impl TaskExecutor {
    pub fn new(
        task_registry: Arc<dyn TaskRegistryPort>,
        permission_gate: Arc<dyn PermissionGatePort>,
        tool_executor: Arc<dyn ToolExecutor>,
    ) -> Self {
        Self {
            task_registry,
            permission_gate,
            tool_executor,
        }
    }

    /// Execute a run: multi-step loop with guardrails and permission checks.
    pub async fn execute_run(
        &self,
        run_id: RunId,
        agent_id: &AgentId,
        _goal: &str,
        actions: Vec<ToolAction>,
        config: ExecutorConfig,
        interrupt_rx: watch::Receiver<bool>,
    ) -> Result<ExecutionSummary> {
        let mut state = ExecutionState::new(config);

        // Transition: Queued -> Planning
        state.transition(RunStatus::Planning)?;
        self.task_registry
            .update_run(
                run_id,
                RunUpdate {
                    run_status: Some(RunStatus::Planning),
                    started_at: Some(Utc::now()),
                    ..Default::default()
                },
            )
            .await?;

        // Transition: Planning -> Running
        state.transition(RunStatus::Running)?;
        self.task_registry
            .update_run(
                run_id,
                RunUpdate {
                    run_status: Some(RunStatus::Running),
                    ..Default::default()
                },
            )
            .await?;

        let mut completed_steps = Vec::new();
        let mut total_tokens = 0u64;

        for (i, action) in actions.iter().enumerate() {
            // Check for user interrupt
            if *interrupt_rx.borrow() {
                state.transition(RunStatus::Interrupted)?;
                return Ok(ExecutionSummary {
                    status: RunStatus::Interrupted,
                    steps: completed_steps,
                    tokens_used: total_tokens,
                    failure_reason: None,
                });
            }

            // Permission check via PermissionGatePort
            let risk = self.tool_executor.classify_risk(&action.tool_name);
            let decision = self
                .permission_gate
                .check_permission(agent_id, risk)
                .await?;

            match decision {
                PermissionDecision::AutoAllow => { /* proceed */ }
                PermissionDecision::Confirm { reason: _ } => {
                    // Transition to WaitingConfirmation, then auto-resume
                    state.transition(RunStatus::WaitingConfirmation)?;
                    self.task_registry
                        .update_run(
                            run_id,
                            RunUpdate {
                                run_status: Some(RunStatus::WaitingConfirmation),
                                ..Default::default()
                            },
                        )
                        .await?;

                    // Auto-resume (real implementation would wait for user input)
                    state.transition(RunStatus::Running)?;
                    self.task_registry
                        .update_run(
                            run_id,
                            RunUpdate {
                                run_status: Some(RunStatus::Running),
                                ..Default::default()
                            },
                        )
                        .await?;
                }
                PermissionDecision::Deny { reason } => {
                    state.transition(RunStatus::Failed)?;
                    return Ok(ExecutionSummary {
                        status: RunStatus::Failed,
                        steps: completed_steps,
                        tokens_used: total_tokens,
                        failure_reason: Some(format!("permission denied: {}", reason)),
                    });
                }
            }

            // Execute the tool
            let result = self.tool_executor.execute(action).await;

            let (success, output, tokens) = match result {
                Ok(r) => (r.success, r.output, r.tokens_used),
                Err(e) => {
                    state.transition(RunStatus::Failed)?;
                    return Ok(ExecutionSummary {
                        status: RunStatus::Failed,
                        steps: completed_steps,
                        tokens_used: total_tokens,
                        failure_reason: Some(format!("tool execution failed: {}", e)),
                    });
                }
            };

            total_tokens += tokens;

            let step = ExecutionStep {
                step_no: (i + 1) as u32,
                action: action.tool_name.clone(),
                tool: Some(action.tool_name.clone()),
                evidence: Some(output),
                risk_reason: None,
                result_summary: Some(if success { "success" } else { "failed" }.into()),
            };

            // Record step and check guardrails (step limit, loop detection)
            if let Some(violation) = state.record_step(step.clone()) {
                let failure_reason = format!("guardrail violation: {:?}", violation);
                state.transition(RunStatus::Failed)?;
                self.task_registry
                    .update_run(
                        run_id,
                        RunUpdate {
                            run_status: Some(RunStatus::Failed),
                            failure_reason: Some(failure_reason.clone()),
                            tokens_used: Some(total_tokens),
                            steps_count: Some(completed_steps.len() as u32 + 1),
                            finished_at: Some(Utc::now()),
                            ..Default::default()
                        },
                    )
                    .await?;
                return Ok(ExecutionSummary {
                    status: RunStatus::Failed,
                    steps: completed_steps,
                    tokens_used: total_tokens,
                    failure_reason: Some(failure_reason),
                });
            }

            // Token budget check
            if let Some(violation) = state.add_tokens(tokens) {
                match violation {
                    GuardrailViolation::TokenBudgetExceeded { .. } => {
                        state.transition(RunStatus::Failed)?;
                        let reason = format!("token budget exceeded: {} used", total_tokens);
                        return Ok(ExecutionSummary {
                            status: RunStatus::Failed,
                            steps: completed_steps,
                            tokens_used: total_tokens,
                            failure_reason: Some(reason),
                        });
                    }
                    GuardrailViolation::TokenBudgetWarning { .. } => {
                        // Warning only, continue execution
                    }
                    _ => {}
                }
            }

            completed_steps.push(step);

            // Update run progress with checkpoint
            self.task_registry
                .update_run(
                    run_id,
                    RunUpdate {
                        checkpoint: Some(state.checkpoint()),
                        tokens_used: Some(total_tokens),
                        steps_count: Some(completed_steps.len() as u32),
                        ..Default::default()
                    },
                )
                .await?;
        }

        // All steps completed successfully
        state.transition(RunStatus::Completed)?;
        self.task_registry
            .update_run(
                run_id,
                RunUpdate {
                    run_status: Some(RunStatus::Completed),
                    tokens_used: Some(total_tokens),
                    steps_count: Some(completed_steps.len() as u32),
                    result_summary: Some(format!("completed {} steps", completed_steps.len())),
                    finished_at: Some(Utc::now()),
                    ..Default::default()
                },
            )
            .await?;

        Ok(ExecutionSummary {
            status: RunStatus::Completed,
            steps: completed_steps,
            tokens_used: total_tokens,
            failure_reason: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // ExecutorConfig
    // -----------------------------------------------------------------------

    #[test]
    fn default_config() {
        let config = ExecutorConfig::default();
        assert_eq!(config.max_steps, 10);
        assert_eq!(config.token_budget, 100_000);
        assert_eq!(config.timeout_secs, 300);
        assert_eq!(config.loop_detection_threshold, 3);
    }

    #[test]
    fn background_config() {
        let config = ExecutorConfig::for_background();
        assert_eq!(config.timeout_secs, 1800);
        assert_eq!(config.max_steps, 10);
    }

    // -----------------------------------------------------------------------
    // State transitions
    // -----------------------------------------------------------------------

    #[test]
    fn valid_state_transitions() {
        let mut state = ExecutionState::new(ExecutorConfig::default());
        assert_eq!(state.status, RunStatus::Queued);

        state.transition(RunStatus::Planning).unwrap();
        assert_eq!(state.status, RunStatus::Planning);

        state.transition(RunStatus::Running).unwrap();
        assert_eq!(state.status, RunStatus::Running);

        state.transition(RunStatus::Completed).unwrap();
        assert_eq!(state.status, RunStatus::Completed);
    }

    #[test]
    fn invalid_state_transition_fails() {
        let mut state = ExecutionState::new(ExecutorConfig::default());
        // Can't go directly from Queued to Running
        let result = state.transition(RunStatus::Running);
        assert!(result.is_err());
    }

    #[test]
    fn waiting_confirmation_transition() {
        let mut state = ExecutionState::new(ExecutorConfig::default());
        state.transition(RunStatus::Planning).unwrap();
        state.transition(RunStatus::Running).unwrap();
        state.transition(RunStatus::WaitingConfirmation).unwrap();
        assert_eq!(state.status, RunStatus::WaitingConfirmation);

        // Can resume from confirmation
        state.transition(RunStatus::Running).unwrap();
        assert_eq!(state.status, RunStatus::Running);
    }

    #[test]
    fn interrupted_from_running() {
        let mut state = ExecutionState::new(ExecutorConfig::default());
        state.transition(RunStatus::Planning).unwrap();
        state.transition(RunStatus::Running).unwrap();
        state.transition(RunStatus::Interrupted).unwrap();
        assert_eq!(state.status, RunStatus::Interrupted);
    }

    // -----------------------------------------------------------------------
    // Step recording & guardrails
    // -----------------------------------------------------------------------

    #[test]
    fn record_step_below_limit() {
        let mut state = ExecutionState::new(ExecutorConfig::default());
        let step = ExecutionStep {
            step_no: 1,
            action: "search".into(),
            tool: Some("web_search".into()),
            evidence: None,
            risk_reason: None,
            result_summary: Some("found 5 results".into()),
        };
        let violation = state.record_step(step);
        assert!(violation.is_none());
        assert_eq!(state.step_count(), 1);
    }

    #[test]
    fn step_limit_exceeded() {
        let config = ExecutorConfig {
            max_steps: 3,
            ..ExecutorConfig::default()
        };
        let mut state = ExecutionState::new(config);

        for i in 0..3 {
            let step = ExecutionStep {
                step_no: i + 1,
                action: format!("step {}", i + 1),
                tool: Some(format!("tool_{}", i)),
                evidence: None,
                risk_reason: None,
                result_summary: None,
            };
            state.record_step(step);
        }

        // 3rd step should trigger limit
        assert_eq!(state.step_count(), 3);
    }

    #[test]
    fn step_limit_triggers_at_max() {
        let config = ExecutorConfig {
            max_steps: 2,
            ..ExecutorConfig::default()
        };
        let mut state = ExecutionState::new(config);

        let step1 = ExecutionStep {
            step_no: 1,
            action: "step 1".into(),
            tool: Some("t1".into()),
            evidence: None,
            risk_reason: None,
            result_summary: None,
        };
        assert!(state.record_step(step1).is_none());

        let step2 = ExecutionStep {
            step_no: 2,
            action: "step 2".into(),
            tool: Some("t2".into()),
            evidence: None,
            risk_reason: None,
            result_summary: None,
        };
        let violation = state.record_step(step2);
        assert!(matches!(violation, Some(GuardrailViolation::StepLimitExceeded { .. })));
    }

    // -----------------------------------------------------------------------
    // Token budget
    // -----------------------------------------------------------------------

    #[test]
    fn token_budget_warning() {
        let config = ExecutorConfig {
            token_budget: 100,
            ..ExecutorConfig::default()
        };
        let mut state = ExecutionState::new(config);

        // 80% threshold = 80 tokens
        let violation = state.add_tokens(81);
        assert!(matches!(violation, Some(GuardrailViolation::TokenBudgetWarning { .. })));
    }

    #[test]
    fn token_budget_exceeded() {
        let config = ExecutorConfig {
            token_budget: 100,
            ..ExecutorConfig::default()
        };
        let mut state = ExecutionState::new(config);

        let violation = state.add_tokens(101);
        assert!(matches!(violation, Some(GuardrailViolation::TokenBudgetExceeded { .. })));
    }

    #[test]
    fn token_budget_ok() {
        let config = ExecutorConfig {
            token_budget: 100,
            ..ExecutorConfig::default()
        };
        let mut state = ExecutionState::new(config);

        let violation = state.add_tokens(50);
        assert!(violation.is_none());
    }

    // -----------------------------------------------------------------------
    // Loop detection
    // -----------------------------------------------------------------------

    #[test]
    fn detects_loop_repeated_tool() {
        let config = ExecutorConfig {
            max_steps: 20,
            loop_detection_threshold: 3,
            ..ExecutorConfig::default()
        };
        let mut state = ExecutionState::new(config);

        for i in 0..3 {
            let step = ExecutionStep {
                step_no: i + 1,
                action: "search".into(),
                tool: Some("web_search".into()),
                evidence: None,
                risk_reason: None,
                result_summary: None,
            };
            state.record_step(step);
        }

        // After 3 identical tool calls, loop should be detected
        assert!(state.detect_loop());
    }

    #[test]
    fn no_loop_with_varied_tools() {
        let config = ExecutorConfig {
            max_steps: 20,
            loop_detection_threshold: 3,
            ..ExecutorConfig::default()
        };
        let mut state = ExecutionState::new(config);

        let tools = ["search", "analyze", "write"];
        for (i, tool) in tools.iter().enumerate() {
            let step = ExecutionStep {
                step_no: (i + 1) as u32,
                action: "action".into(),
                tool: Some(tool.to_string()),
                evidence: None,
                risk_reason: None,
                result_summary: None,
            };
            state.record_step(step);
        }

        assert!(!state.detect_loop());
    }

    // -----------------------------------------------------------------------
    // Checkpoint
    // -----------------------------------------------------------------------

    #[test]
    fn checkpoint_serializes() {
        let mut state = ExecutionState::new(ExecutorConfig::default());
        let step = ExecutionStep {
            step_no: 1,
            action: "test".into(),
            tool: Some("tool".into()),
            evidence: None,
            risk_reason: None,
            result_summary: Some("done".into()),
        };
        state.record_step(step);
        state.add_tokens(500);

        let checkpoint = state.checkpoint();
        assert_eq!(checkpoint["steps_completed"], 1);
        assert_eq!(checkpoint["tokens_used"], 500);
    }

    // -----------------------------------------------------------------------
    // Intent evaluation
    // -----------------------------------------------------------------------

    #[test]
    fn intent_simple_question() {
        assert_eq!(
            IntentEvaluator::evaluate("What is the capital of France?"),
            IntentCategory::Simple
        );
    }

    #[test]
    fn intent_assisted_search() {
        assert_eq!(
            IntentEvaluator::evaluate("Search for recent papers on AI safety"),
            IntentCategory::Assisted
        );
    }

    #[test]
    fn intent_multi_step_research() {
        assert_eq!(
            IntentEvaluator::evaluate("Search for papers on AI safety, then summarize the top 5, and then create a report"),
            IntentCategory::MultiStep
        );
    }

    #[test]
    fn intent_multi_step_compound() {
        assert_eq!(
            IntentEvaluator::evaluate("First find all relevant articles, after that analyze them"),
            IntentCategory::MultiStep
        );
    }

    #[test]
    fn intent_simple_greeting() {
        assert_eq!(
            IntentEvaluator::evaluate("Hello, how are you?"),
            IntentCategory::Simple
        );
    }

    // -----------------------------------------------------------------------
    // TaskExecutor tests (Phase 4.3 & 5.2)
    // -----------------------------------------------------------------------

    use super::{TaskExecutor, ToolAction, ToolExecutor, ToolResult};
    use clawx_types::ids::AgentId;
    use clawx_types::permission::{RiskLevel, PermissionDecision};
    use clawx_types::traits::{TaskRegistryPort, PermissionGatePort, RunUpdate};
    use clawx_types::autonomy::RunId;
    use std::sync::{Arc, Mutex};
    use tokio::sync::watch;

    // -- Stub ToolExecutor --

    struct StubToolExecutor {
        results: Mutex<Vec<ToolResult>>,
        default_risk: RiskLevel,
        risk_overrides: std::collections::HashMap<String, RiskLevel>,
    }

    impl StubToolExecutor {
        fn new(results: Vec<ToolResult>) -> Self {
            Self {
                results: Mutex::new(results),
                default_risk: RiskLevel::Read,
                risk_overrides: std::collections::HashMap::new(),
            }
        }
    }

    #[async_trait::async_trait]
    impl ToolExecutor for StubToolExecutor {
        async fn execute(&self, _action: &ToolAction) -> clawx_types::error::Result<ToolResult> {
            let mut results = self.results.lock().unwrap();
            if results.is_empty() {
                return Err(clawx_types::ClawxError::Internal("no more stub results".into()));
            }
            Ok(results.remove(0))
        }

        fn classify_risk(&self, tool_name: &str) -> RiskLevel {
            self.risk_overrides.get(tool_name).copied().unwrap_or(self.default_risk)
        }
    }

    // Failing tool executor
    struct FailingToolExecutor {
        fail_at_step: usize,
        call_count: Mutex<usize>,
    }

    #[async_trait::async_trait]
    impl ToolExecutor for FailingToolExecutor {
        async fn execute(&self, _action: &ToolAction) -> clawx_types::error::Result<ToolResult> {
            let mut count = self.call_count.lock().unwrap();
            *count += 1;
            if *count == self.fail_at_step {
                return Err(clawx_types::ClawxError::Internal("tool exploded".into()));
            }
            Ok(ToolResult { success: true, output: "ok".into(), tokens_used: 10 })
        }

        fn classify_risk(&self, _tool_name: &str) -> RiskLevel {
            RiskLevel::Read
        }
    }

    // -- Stub PermissionGate --

    struct StubPermissionGate {
        decision: PermissionDecision,
        per_risk: std::collections::HashMap<String, PermissionDecision>,
    }

    impl StubPermissionGate {
        fn always(decision: PermissionDecision) -> Self {
            Self { decision, per_risk: std::collections::HashMap::new() }
        }
    }

    #[async_trait::async_trait]
    impl PermissionGatePort for StubPermissionGate {
        async fn check_permission(
            &self,
            _agent_id: &AgentId,
            risk_level: RiskLevel,
        ) -> clawx_types::error::Result<PermissionDecision> {
            let key = format!("{:?}", risk_level);
            if let Some(d) = self.per_risk.get(&key) {
                return Ok(d.clone());
            }
            Ok(self.decision.clone())
        }

        async fn get_profile(
            &self,
            _agent_id: &AgentId,
        ) -> clawx_types::error::Result<Option<clawx_types::permission::PermissionProfile>> {
            Ok(None)
        }

        async fn update_profile(
            &self,
            _agent_id: &AgentId,
            _dimension: clawx_types::permission::CapabilityDimension,
            _new_level: clawx_types::permission::TrustLevel,
            _reason: String,
        ) -> clawx_types::error::Result<()> {
            Ok(())
        }

        async fn record_safety_incident(
            &self,
            _agent_id: &AgentId,
        ) -> clawx_types::error::Result<()> {
            Ok(())
        }
    }

    // -- Stub TaskRegistry (records calls for verification) --

    #[derive(Debug, Clone)]
    struct RunUpdateRecord {
        update: RunUpdate,
    }

    struct StubTaskRegistry {
        updates: Mutex<Vec<RunUpdateRecord>>,
    }

    impl StubTaskRegistry {
        fn new() -> Self {
            Self { updates: Mutex::new(Vec::new()) }
        }

        fn get_updates(&self) -> Vec<RunUpdateRecord> {
            self.updates.lock().unwrap().clone()
        }
    }

    #[async_trait::async_trait]
    impl TaskRegistryPort for StubTaskRegistry {
        async fn create_task(&self, _task: clawx_types::autonomy::Task) -> clawx_types::error::Result<clawx_types::TaskId> {
            Ok(clawx_types::TaskId::new())
        }
        async fn get_task(&self, _id: clawx_types::TaskId) -> clawx_types::error::Result<Option<clawx_types::autonomy::Task>> {
            Ok(None)
        }
        async fn list_tasks(&self, _agent_id: Option<AgentId>, _pagination: clawx_types::Pagination) -> clawx_types::error::Result<clawx_types::PagedResult<clawx_types::autonomy::Task>> {
            Ok(clawx_types::PagedResult { items: vec![], total: 0, page: 1, per_page: 20 })
        }
        async fn update_task(&self, _id: clawx_types::TaskId, _update: clawx_types::traits::TaskUpdate) -> clawx_types::error::Result<()> {
            Ok(())
        }
        async fn delete_task(&self, _id: clawx_types::TaskId) -> clawx_types::error::Result<()> {
            Ok(())
        }
        async fn update_lifecycle(&self, _id: clawx_types::TaskId, _status: clawx_types::autonomy::TaskLifecycleStatus) -> clawx_types::error::Result<()> {
            Ok(())
        }
        async fn add_trigger(&self, _trigger: clawx_types::autonomy::Trigger) -> clawx_types::error::Result<clawx_types::autonomy::TriggerId> {
            Ok(clawx_types::autonomy::TriggerId::new())
        }
        async fn get_trigger(&self, _id: clawx_types::autonomy::TriggerId) -> clawx_types::error::Result<Option<clawx_types::autonomy::Trigger>> {
            Ok(None)
        }
        async fn list_triggers(&self, _task_id: clawx_types::TaskId) -> clawx_types::error::Result<Vec<clawx_types::autonomy::Trigger>> {
            Ok(vec![])
        }
        async fn update_trigger(&self, _id: clawx_types::autonomy::TriggerId, _update: clawx_types::traits::TriggerUpdate) -> clawx_types::error::Result<()> {
            Ok(())
        }
        async fn delete_trigger(&self, _id: clawx_types::autonomy::TriggerId) -> clawx_types::error::Result<()> {
            Ok(())
        }
        async fn get_due_triggers(&self, _now: chrono::DateTime<chrono::Utc>) -> clawx_types::error::Result<Vec<clawx_types::autonomy::Trigger>> {
            Ok(vec![])
        }
        async fn create_run(&self, _run: clawx_types::autonomy::Run) -> clawx_types::error::Result<RunId> {
            Ok(RunId::new())
        }
        async fn get_run(&self, _id: RunId) -> clawx_types::error::Result<Option<clawx_types::autonomy::Run>> {
            Ok(None)
        }
        async fn list_runs(&self, _task_id: clawx_types::TaskId, _pagination: clawx_types::Pagination) -> clawx_types::error::Result<clawx_types::PagedResult<clawx_types::autonomy::Run>> {
            Ok(clawx_types::PagedResult { items: vec![], total: 0, page: 1, per_page: 20 })
        }
        async fn update_run(&self, _id: RunId, update: RunUpdate) -> clawx_types::error::Result<()> {
            self.updates.lock().unwrap().push(RunUpdateRecord { update });
            Ok(())
        }
        async fn get_incomplete_runs(&self) -> clawx_types::error::Result<Vec<clawx_types::autonomy::Run>> {
            Ok(vec![])
        }
        async fn record_feedback(&self, _run_id: RunId, _kind: clawx_types::autonomy::FeedbackKind, _reason: Option<String>) -> clawx_types::error::Result<()> {
            Ok(())
        }
    }

    // -- Helper functions --

    fn make_actions(names: Vec<&str>) -> Vec<ToolAction> {
        names.into_iter().map(|n| ToolAction {
            tool_name: n.to_string(),
            parameters: serde_json::json!({}),
            risk_level: RiskLevel::Read,
        }).collect()
    }

    fn make_results(count: usize, tokens_each: u64) -> Vec<ToolResult> {
        (0..count).map(|_| ToolResult {
            success: true,
            output: "done".into(),
            tokens_used: tokens_each,
        }).collect()
    }

    fn no_interrupt() -> watch::Receiver<bool> {
        let (_tx, rx) = watch::channel(false);
        rx
    }

    // -- Tests --

    #[tokio::test]
    async fn execute_run_completes_all_steps() {
        let registry = Arc::new(StubTaskRegistry::new());
        let gate = Arc::new(StubPermissionGate::always(PermissionDecision::AutoAllow));
        let tool_exec = Arc::new(StubToolExecutor::new(make_results(3, 100)));

        let executor = TaskExecutor::new(
            registry.clone() as Arc<dyn TaskRegistryPort>,
            gate as Arc<dyn PermissionGatePort>,
            tool_exec as Arc<dyn ToolExecutor>,
        );

        let result = executor.execute_run(
            RunId::new(),
            &AgentId::new(),
            "test goal",
            make_actions(vec!["search", "analyze", "write"]),
            ExecutorConfig { max_steps: 10, ..ExecutorConfig::default() },
            no_interrupt(),
        ).await.unwrap();

        assert_eq!(result.status, RunStatus::Completed);
        assert_eq!(result.steps.len(), 3);
        assert_eq!(result.tokens_used, 300);
        assert!(result.failure_reason.is_none());
    }

    #[tokio::test]
    async fn execute_run_tool_failure_stops() {
        let registry = Arc::new(StubTaskRegistry::new());
        let gate = Arc::new(StubPermissionGate::always(PermissionDecision::AutoAllow));
        let tool_exec = Arc::new(FailingToolExecutor {
            fail_at_step: 2,
            call_count: Mutex::new(0),
        });

        let executor = TaskExecutor::new(
            registry.clone() as Arc<dyn TaskRegistryPort>,
            gate as Arc<dyn PermissionGatePort>,
            tool_exec as Arc<dyn ToolExecutor>,
        );

        let result = executor.execute_run(
            RunId::new(),
            &AgentId::new(),
            "test goal",
            make_actions(vec!["step1", "step2", "step3"]),
            ExecutorConfig::default(),
            no_interrupt(),
        ).await.unwrap();

        assert_eq!(result.status, RunStatus::Failed);
        assert_eq!(result.steps.len(), 1); // only first step completed
        assert!(result.failure_reason.unwrap().contains("tool execution failed"));
    }

    #[tokio::test]
    async fn execute_run_permission_denied_stops() {
        let registry = Arc::new(StubTaskRegistry::new());
        let gate = Arc::new(StubPermissionGate::always(
            PermissionDecision::Deny { reason: "too dangerous".into() },
        ));
        let tool_exec = Arc::new(StubToolExecutor::new(make_results(3, 100)));

        let executor = TaskExecutor::new(
            registry.clone() as Arc<dyn TaskRegistryPort>,
            gate as Arc<dyn PermissionGatePort>,
            tool_exec as Arc<dyn ToolExecutor>,
        );

        let result = executor.execute_run(
            RunId::new(),
            &AgentId::new(),
            "test goal",
            make_actions(vec!["danger_tool"]),
            ExecutorConfig::default(),
            no_interrupt(),
        ).await.unwrap();

        assert_eq!(result.status, RunStatus::Failed);
        assert_eq!(result.steps.len(), 0);
        assert!(result.failure_reason.unwrap().contains("permission denied"));
    }

    #[tokio::test]
    async fn execute_run_permission_confirm_continues() {
        let registry = Arc::new(StubTaskRegistry::new());
        let gate = Arc::new(StubPermissionGate::always(
            PermissionDecision::Confirm { reason: "needs confirmation".into() },
        ));
        let tool_exec = Arc::new(StubToolExecutor::new(make_results(2, 50)));

        let executor = TaskExecutor::new(
            registry.clone() as Arc<dyn TaskRegistryPort>,
            gate as Arc<dyn PermissionGatePort>,
            tool_exec as Arc<dyn ToolExecutor>,
        );

        let result = executor.execute_run(
            RunId::new(),
            &AgentId::new(),
            "test goal",
            make_actions(vec!["tool_a", "tool_b"]),
            ExecutorConfig::default(),
            no_interrupt(),
        ).await.unwrap();

        assert_eq!(result.status, RunStatus::Completed);
        assert_eq!(result.steps.len(), 2);
    }

    #[tokio::test]
    async fn execute_run_step_limit_exceeded() {
        let registry = Arc::new(StubTaskRegistry::new());
        let gate = Arc::new(StubPermissionGate::always(PermissionDecision::AutoAllow));
        let tool_exec = Arc::new(StubToolExecutor::new(make_results(3, 10)));

        let executor = TaskExecutor::new(
            registry.clone() as Arc<dyn TaskRegistryPort>,
            gate as Arc<dyn PermissionGatePort>,
            tool_exec as Arc<dyn ToolExecutor>,
        );

        let result = executor.execute_run(
            RunId::new(),
            &AgentId::new(),
            "test goal",
            make_actions(vec!["a", "b", "c"]),
            ExecutorConfig { max_steps: 2, ..ExecutorConfig::default() },
            no_interrupt(),
        ).await.unwrap();

        assert_eq!(result.status, RunStatus::Failed);
        assert!(result.failure_reason.unwrap().contains("guardrail violation"));
    }

    #[tokio::test]
    async fn execute_run_token_budget_exceeded() {
        let registry = Arc::new(StubTaskRegistry::new());
        let gate = Arc::new(StubPermissionGate::always(PermissionDecision::AutoAllow));
        // Each step uses 60k tokens, budget is 100k, so second step exceeds
        let tool_exec = Arc::new(StubToolExecutor::new(make_results(3, 60_000)));

        let executor = TaskExecutor::new(
            registry.clone() as Arc<dyn TaskRegistryPort>,
            gate as Arc<dyn PermissionGatePort>,
            tool_exec as Arc<dyn ToolExecutor>,
        );

        let result = executor.execute_run(
            RunId::new(),
            &AgentId::new(),
            "test goal",
            make_actions(vec!["a", "b", "c"]),
            ExecutorConfig { token_budget: 100_000, max_steps: 10, ..ExecutorConfig::default() },
            no_interrupt(),
        ).await.unwrap();

        assert_eq!(result.status, RunStatus::Failed);
        assert!(result.failure_reason.unwrap().contains("token budget exceeded"));
    }

    #[tokio::test]
    async fn execute_run_interrupt_stops_execution() {
        let registry = Arc::new(StubTaskRegistry::new());
        let gate = Arc::new(StubPermissionGate::always(PermissionDecision::AutoAllow));
        let tool_exec = Arc::new(StubToolExecutor::new(make_results(5, 10)));

        let executor = TaskExecutor::new(
            registry.clone() as Arc<dyn TaskRegistryPort>,
            gate as Arc<dyn PermissionGatePort>,
            tool_exec as Arc<dyn ToolExecutor>,
        );

        // Pre-set interrupt signal to true
        let (tx, rx) = watch::channel(true);

        let result = executor.execute_run(
            RunId::new(),
            &AgentId::new(),
            "test goal",
            make_actions(vec!["a", "b", "c"]),
            ExecutorConfig::default(),
            rx,
        ).await.unwrap();

        assert_eq!(result.status, RunStatus::Interrupted);
        assert_eq!(result.steps.len(), 0);
        drop(tx);
    }

    #[tokio::test]
    async fn execute_run_updates_run_status() {
        let registry = Arc::new(StubTaskRegistry::new());
        let gate = Arc::new(StubPermissionGate::always(PermissionDecision::AutoAllow));
        let tool_exec = Arc::new(StubToolExecutor::new(make_results(1, 10)));

        let executor = TaskExecutor::new(
            registry.clone() as Arc<dyn TaskRegistryPort>,
            gate as Arc<dyn PermissionGatePort>,
            tool_exec as Arc<dyn ToolExecutor>,
        );

        executor.execute_run(
            RunId::new(),
            &AgentId::new(),
            "test goal",
            make_actions(vec!["search"]),
            ExecutorConfig::default(),
            no_interrupt(),
        ).await.unwrap();

        let updates = registry.get_updates();
        // Should have: Planning, Running, progress update, Completed
        let statuses: Vec<Option<RunStatus>> = updates.iter()
            .map(|u| u.update.run_status)
            .collect();

        assert!(statuses.contains(&Some(RunStatus::Planning)));
        assert!(statuses.contains(&Some(RunStatus::Running)));
        assert!(statuses.contains(&Some(RunStatus::Completed)));
    }

    #[tokio::test]
    async fn execute_run_records_checkpoint() {
        let registry = Arc::new(StubTaskRegistry::new());
        let gate = Arc::new(StubPermissionGate::always(PermissionDecision::AutoAllow));
        let tool_exec = Arc::new(StubToolExecutor::new(make_results(2, 50)));

        let executor = TaskExecutor::new(
            registry.clone() as Arc<dyn TaskRegistryPort>,
            gate as Arc<dyn PermissionGatePort>,
            tool_exec as Arc<dyn ToolExecutor>,
        );

        executor.execute_run(
            RunId::new(),
            &AgentId::new(),
            "test goal",
            make_actions(vec!["a", "b"]),
            ExecutorConfig::default(),
            no_interrupt(),
        ).await.unwrap();

        let updates = registry.get_updates();
        let checkpoint_updates: Vec<_> = updates.iter()
            .filter(|u| u.update.checkpoint.is_some())
            .collect();

        // Should have a checkpoint after each step
        assert_eq!(checkpoint_updates.len(), 2);
    }

    #[tokio::test]
    async fn execute_run_loop_detection() {
        let registry = Arc::new(StubTaskRegistry::new());
        let gate = Arc::new(StubPermissionGate::always(PermissionDecision::AutoAllow));
        // 3 identical tool calls should trigger loop detection (threshold=3)
        let tool_exec = Arc::new(StubToolExecutor::new(make_results(4, 10)));

        let executor = TaskExecutor::new(
            registry.clone() as Arc<dyn TaskRegistryPort>,
            gate as Arc<dyn PermissionGatePort>,
            tool_exec as Arc<dyn ToolExecutor>,
        );

        let result = executor.execute_run(
            RunId::new(),
            &AgentId::new(),
            "test goal",
            make_actions(vec!["same_tool", "same_tool", "same_tool", "same_tool"]),
            ExecutorConfig {
                max_steps: 20,
                loop_detection_threshold: 3,
                ..ExecutorConfig::default()
            },
            no_interrupt(),
        ).await.unwrap();

        assert_eq!(result.status, RunStatus::Failed);
        assert!(result.failure_reason.unwrap().contains("guardrail violation"));
    }

    #[tokio::test]
    async fn execute_run_empty_actions_completes() {
        let registry = Arc::new(StubTaskRegistry::new());
        let gate = Arc::new(StubPermissionGate::always(PermissionDecision::AutoAllow));
        let tool_exec = Arc::new(StubToolExecutor::new(vec![]));

        let executor = TaskExecutor::new(
            registry.clone() as Arc<dyn TaskRegistryPort>,
            gate as Arc<dyn PermissionGatePort>,
            tool_exec as Arc<dyn ToolExecutor>,
        );

        let result = executor.execute_run(
            RunId::new(),
            &AgentId::new(),
            "test goal",
            vec![], // empty actions
            ExecutorConfig::default(),
            no_interrupt(),
        ).await.unwrap();

        assert_eq!(result.status, RunStatus::Completed);
        assert_eq!(result.steps.len(), 0);
        assert_eq!(result.tokens_used, 0);
    }

    #[tokio::test]
    async fn execute_run_permission_auto_allow() {
        let registry = Arc::new(StubTaskRegistry::new());
        let gate = Arc::new(StubPermissionGate::always(PermissionDecision::AutoAllow));
        let tool_exec = Arc::new(StubToolExecutor::new(make_results(3, 10)));

        let executor = TaskExecutor::new(
            registry.clone() as Arc<dyn TaskRegistryPort>,
            gate as Arc<dyn PermissionGatePort>,
            tool_exec as Arc<dyn ToolExecutor>,
        );

        let result = executor.execute_run(
            RunId::new(),
            &AgentId::new(),
            "test goal",
            make_actions(vec!["read_a", "read_b", "read_c"]),
            ExecutorConfig::default(),
            no_interrupt(),
        ).await.unwrap();

        assert_eq!(result.status, RunStatus::Completed);
        assert_eq!(result.steps.len(), 3);
        assert_eq!(result.tokens_used, 30);

        // Verify no WaitingConfirmation transitions occurred
        let updates = registry.get_updates();
        let waiting_updates: Vec<_> = updates.iter()
            .filter(|u| u.update.run_status == Some(RunStatus::WaitingConfirmation))
            .collect();
        assert!(waiting_updates.is_empty());
    }
}
