//! Agent lifecycle, Agent Loop, and task dispatcher for ClawX.
//!
//! Orchestrates the core agent loop: receive task, plan, execute tools,
//! observe results, and iterate until the task is complete or a limit
//! is reached.

/// The main agent loop implementation.
pub mod agent_loop;

/// Task dispatcher for routing work to agents.
pub mod dispatcher;

/// Agent lifecycle management (spawn, pause, resume, stop).
pub mod lifecycle;
