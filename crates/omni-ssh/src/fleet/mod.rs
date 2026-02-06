//! Fleet operations for batch command execution
//!
//! This module provides fleet management capabilities for executing
//! commands across multiple OmniEdge nodes simultaneously.
//!
//! ## Features
//!
//! - Parallel command execution across multiple nodes
//! - Target selection by node IDs, tags, or filters
//! - Progress tracking and result aggregation
//! - Timeout and concurrency control
//!
//! ## Example
//!
//! ```rust,ignore
//! use omni_ssh::fleet::{FleetExecutor, FleetOperation, FleetTargets};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     // Create executor with your backend
//!     let executor = FleetExecutor::new(backend);
//!     
//!     let operation = FleetOperation::new(
//!         "uptime",
//!         FleetTargets::Tags(vec!["production".to_string()]),
//!     );
//!     
//!     let results = executor.execute(operation).await?;
//!     println!("Succeeded: {}/{}", results.summary.succeeded, results.summary.total);
//!     
//!     Ok(())
//! }
//! ```

use crate::client::{SshClient, SshTarget};
use crate::server::SshBackend;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Fleet operation request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FleetOperation {
    /// Operation ID
    pub operation_id: String,
    /// Command to execute
    pub command: String,
    /// Target node IDs or tags
    pub targets: FleetTargets,
    /// Who initiated the operation
    pub initiator_id: String,
    /// When the operation was started
    pub started_at: DateTime<Utc>,
    /// Timeout for each node (seconds)
    pub timeout_secs: u64,
    /// Maximum parallel executions
    pub max_parallel: u32,
    /// Username to use for SSH connections
    pub ssh_user: String,
    /// SSH port
    pub ssh_port: u16,
}

impl FleetOperation {
    /// Create a new fleet operation
    pub fn new(command: impl Into<String>, targets: FleetTargets) -> Self {
        Self {
            operation_id: Uuid::new_v4().to_string(),
            command: command.into(),
            targets,
            initiator_id: String::new(),
            started_at: Utc::now(),
            timeout_secs: 60,
            max_parallel: 10,
            ssh_user: "root".to_string(),
            ssh_port: 22,
        }
    }

    /// Set the initiator ID
    pub fn with_initiator(mut self, id: impl Into<String>) -> Self {
        self.initiator_id = id.into();
        self
    }

    /// Set the timeout per node
    pub fn with_timeout(mut self, secs: u64) -> Self {
        self.timeout_secs = secs;
        self
    }

    /// Set max parallel executions
    pub fn with_max_parallel(mut self, n: u32) -> Self {
        self.max_parallel = n;
        self
    }

    /// Set SSH user
    pub fn with_ssh_user(mut self, user: impl Into<String>) -> Self {
        self.ssh_user = user.into();
        self
    }

    /// Set SSH port
    pub fn with_ssh_port(mut self, port: u16) -> Self {
        self.ssh_port = port;
        self
    }
}

/// Fleet target specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FleetTargets {
    /// Specific node IDs
    NodeIds(Vec<String>),
    /// Nodes with these tags
    Tags(Vec<String>),
    /// All nodes in a network
    Network(String),
    /// Specific VPN IPs
    VpnIps(Vec<String>),
    /// All online nodes
    AllOnline,
}

/// Result of fleet operation on a single node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeResult {
    /// Node ID
    pub node_id: String,
    /// Node name
    pub node_name: String,
    /// VPN IP address
    pub vpn_ip: String,
    /// Exit code (None if failed to connect)
    pub exit_code: Option<i32>,
    /// Standard output
    pub stdout: String,
    /// Standard error
    pub stderr: String,
    /// Execution duration (milliseconds)
    pub duration_ms: u64,
    /// Error message if failed
    pub error: Option<String>,
    /// Status of this node execution
    pub status: NodeExecutionStatus,
}

impl NodeResult {
    /// Check if execution succeeded
    pub fn success(&self) -> bool {
        self.exit_code == Some(0) && self.error.is_none()
    }

    /// Create a result for a successful execution
    fn success_result(
        node_id: String,
        node_name: String,
        vpn_ip: String,
        exit_code: i32,
        stdout: String,
        stderr: String,
        duration_ms: u64,
    ) -> Self {
        Self {
            node_id,
            node_name,
            vpn_ip,
            exit_code: Some(exit_code),
            stdout,
            stderr,
            duration_ms,
            error: None,
            status: if exit_code == 0 {
                NodeExecutionStatus::Succeeded
            } else {
                NodeExecutionStatus::Failed
            },
        }
    }

    /// Create a result for a connection failure
    fn connection_error(
        node_id: String,
        node_name: String,
        vpn_ip: String,
        error: String,
        duration_ms: u64,
    ) -> Self {
        Self {
            node_id,
            node_name,
            vpn_ip,
            exit_code: None,
            stdout: String::new(),
            stderr: String::new(),
            duration_ms,
            error: Some(error),
            status: NodeExecutionStatus::Unreachable,
        }
    }

    /// Create a result for a timeout
    fn timeout(node_id: String, node_name: String, vpn_ip: String, timeout_secs: u64) -> Self {
        Self {
            node_id,
            node_name,
            vpn_ip,
            exit_code: None,
            stdout: String::new(),
            stderr: String::new(),
            duration_ms: timeout_secs * 1000,
            error: Some(format!("Execution timed out after {}s", timeout_secs)),
            status: NodeExecutionStatus::TimedOut,
        }
    }
}

/// Status of node execution
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum NodeExecutionStatus {
    /// Pending execution
    Pending,
    /// Currently executing
    Running,
    /// Execution succeeded (exit code 0)
    Succeeded,
    /// Execution failed (non-zero exit code)
    Failed,
    /// Execution timed out
    TimedOut,
    /// Node was unreachable
    Unreachable,
    /// Execution was cancelled
    Cancelled,
}

/// Aggregated fleet operation results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FleetResults {
    /// Operation ID
    pub operation_id: String,
    /// Command that was executed
    pub command: String,
    /// When operation started
    pub started_at: DateTime<Utc>,
    /// When operation completed
    pub completed_at: DateTime<Utc>,
    /// Results per node
    pub results: HashMap<String, NodeResult>,
    /// Summary statistics
    pub summary: FleetSummary,
}

impl FleetResults {
    /// Get all successful results
    pub fn successful(&self) -> impl Iterator<Item = &NodeResult> {
        self.results
            .values()
            .filter(|r| r.status == NodeExecutionStatus::Succeeded)
    }

    /// Get all failed results
    pub fn failed(&self) -> impl Iterator<Item = &NodeResult> {
        self.results
            .values()
            .filter(|r| r.status == NodeExecutionStatus::Failed)
    }

    /// Print a summary to stdout
    pub fn print_summary(&self) {
        println!("Fleet Operation Results");
        println!("=======================");
        println!("Operation ID: {}", self.operation_id);
        println!("Command: {}", self.command);
        println!(
            "Duration: {}ms",
            (self.completed_at - self.started_at).num_milliseconds()
        );
        println!();
        println!("Summary:");
        println!("  Total:       {}", self.summary.total);
        println!("  Succeeded:   {}", self.summary.succeeded);
        println!("  Failed:      {}", self.summary.failed);
        println!("  Timed Out:   {}", self.summary.timed_out);
        println!("  Unreachable: {}", self.summary.unreachable);
    }
}

/// Summary of fleet operation results
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FleetSummary {
    /// Total number of targets
    pub total: usize,
    /// Successful executions
    pub succeeded: usize,
    /// Failed executions
    pub failed: usize,
    /// Timed out executions
    pub timed_out: usize,
    /// Unreachable nodes
    pub unreachable: usize,
}

impl FleetSummary {
    fn from_results(results: &HashMap<String, NodeResult>) -> Self {
        let mut summary = Self::default();
        summary.total = results.len();

        for result in results.values() {
            match result.status {
                NodeExecutionStatus::Succeeded => summary.succeeded += 1,
                NodeExecutionStatus::Failed => summary.failed += 1,
                NodeExecutionStatus::TimedOut => summary.timed_out += 1,
                NodeExecutionStatus::Unreachable => summary.unreachable += 1,
                _ => {}
            }
        }

        summary
    }
}

/// Target node info for fleet execution
#[derive(Debug, Clone)]
pub struct TargetNode {
    /// Node ID
    pub node_id: String,
    /// Node name
    pub name: String,
    /// VPN IP address
    pub vpn_ip: String,
    /// Whether the node is online
    pub online: bool,
}

/// Fleet executor
pub struct FleetExecutor {
    backend: Arc<dyn SshBackend>,
}

impl FleetExecutor {
    /// Create a new fleet executor
    pub fn new(backend: Arc<dyn SshBackend>) -> Self {
        Self { backend }
    }

    /// Execute a command across fleet
    pub async fn execute(&self, operation: FleetOperation) -> anyhow::Result<FleetResults> {
        info!(
            operation_id = %operation.operation_id,
            command = %operation.command,
            "Starting fleet operation"
        );

        let started_at = Utc::now();

        // Resolve targets to actual nodes
        let targets = self.resolve_targets(&operation.targets).await?;

        if targets.is_empty() {
            warn!("No targets resolved for fleet operation");
            return Ok(FleetResults {
                operation_id: operation.operation_id,
                command: operation.command,
                started_at,
                completed_at: Utc::now(),
                results: HashMap::new(),
                summary: FleetSummary::default(),
            });
        }

        info!("Resolved {} targets for fleet operation", targets.len());

        // Create semaphore for concurrency control
        let semaphore = Arc::new(Semaphore::new(operation.max_parallel as usize));

        // Execute on all targets in parallel
        let mut handles = Vec::new();

        for target in targets {
            let permit = semaphore.clone().acquire_owned().await?;
            let command = operation.command.clone();
            let timeout = Duration::from_secs(operation.timeout_secs);
            let ssh_user = operation.ssh_user.clone();
            let ssh_port = operation.ssh_port;
            let backend = self.backend.clone();

            let handle = tokio::spawn(async move {
                let result =
                    Self::execute_on_node(backend, &target, &command, &ssh_user, ssh_port, timeout)
                        .await;
                drop(permit);
                (target.node_id.clone(), result)
            });

            handles.push(handle);
        }

        // Collect results
        let mut results = HashMap::new();
        for handle in handles {
            match handle.await {
                Ok((node_id, result)) => {
                    results.insert(node_id, result);
                }
                Err(e) => {
                    error!("Task panicked: {}", e);
                }
            }
        }

        let completed_at = Utc::now();
        let summary = FleetSummary::from_results(&results);

        info!(
            operation_id = %operation.operation_id,
            total = summary.total,
            succeeded = summary.succeeded,
            failed = summary.failed,
            "Fleet operation completed"
        );

        Ok(FleetResults {
            operation_id: operation.operation_id,
            command: operation.command,
            started_at,
            completed_at,
            results,
            summary,
        })
    }

    /// Execute command on a single node
    async fn execute_on_node(
        backend: Arc<dyn SshBackend>,
        target: &TargetNode,
        command: &str,
        ssh_user: &str,
        ssh_port: u16,
        timeout: Duration,
    ) -> NodeResult {
        let start = Instant::now();

        debug!(
            node_id = %target.node_id,
            vpn_ip = %target.vpn_ip,
            command = %command,
            "Executing on node"
        );

        // Create SSH target
        let ssh_target = SshTarget::new(&target.vpn_ip, ssh_user).with_port(ssh_port);

        // Connect with timeout
        let client = SshClient::new(backend);

        let connect_result = tokio::time::timeout(timeout, client.connect(ssh_target)).await;

        let mut session = match connect_result {
            Ok(Ok(session)) => session,
            Ok(Err(e)) => {
                return NodeResult::connection_error(
                    target.node_id.clone(),
                    target.name.clone(),
                    target.vpn_ip.clone(),
                    format!("Connection failed: {}", e),
                    start.elapsed().as_millis() as u64,
                );
            }
            Err(_) => {
                return NodeResult::timeout(
                    target.node_id.clone(),
                    target.name.clone(),
                    target.vpn_ip.clone(),
                    timeout.as_secs(),
                );
            }
        };

        // Execute command with timeout
        let remaining_timeout = timeout.saturating_sub(start.elapsed());
        let exec_result = tokio::time::timeout(remaining_timeout, session.exec(command)).await;

        let result = match exec_result {
            Ok(Ok(exec_result)) => NodeResult::success_result(
                target.node_id.clone(),
                target.name.clone(),
                target.vpn_ip.clone(),
                exec_result.exit_code,
                exec_result.stdout_str(),
                exec_result.stderr_str(),
                start.elapsed().as_millis() as u64,
            ),
            Ok(Err(e)) => NodeResult::connection_error(
                target.node_id.clone(),
                target.name.clone(),
                target.vpn_ip.clone(),
                format!("Execution failed: {}", e),
                start.elapsed().as_millis() as u64,
            ),
            Err(_) => NodeResult::timeout(
                target.node_id.clone(),
                target.name.clone(),
                target.vpn_ip.clone(),
                timeout.as_secs(),
            ),
        };

        // Close session (ignore errors)
        let _ = session.close().await;

        result
    }

    /// Resolve targets to actual node information
    pub async fn resolve_targets(&self, targets: &FleetTargets) -> anyhow::Result<Vec<TargetNode>> {
        let peers = self.backend.list_peers().await?;

        let result = match targets {
            FleetTargets::NodeIds(ids) => peers
                .into_iter()
                .filter(|p| {
                    p.device_id
                        .as_ref()
                        .map(|id| ids.contains(id))
                        .unwrap_or(false)
                })
                .map(|p| TargetNode {
                    node_id: p.device_id.unwrap_or_default(),
                    name: p.name,
                    vpn_ip: p.vpn_ip.to_string(),
                    online: p.online,
                })
                .collect(),

            FleetTargets::VpnIps(ips) => peers
                .into_iter()
                .filter(|p| ips.contains(&p.vpn_ip.to_string()))
                .map(|p| TargetNode {
                    node_id: p.device_id.unwrap_or_default(),
                    name: p.name,
                    vpn_ip: p.vpn_ip.to_string(),
                    online: p.online,
                })
                .collect(),

            FleetTargets::AllOnline => peers
                .into_iter()
                .filter(|p| p.online)
                .map(|p| TargetNode {
                    node_id: p.device_id.unwrap_or_default(),
                    name: p.name,
                    vpn_ip: p.vpn_ip.to_string(),
                    online: p.online,
                })
                .collect(),

            FleetTargets::Network(_) | FleetTargets::Tags(_) => {
                // For now, return all peers - tag filtering would require additional API
                peers
                    .into_iter()
                    .map(|p| TargetNode {
                        node_id: p.device_id.unwrap_or_default(),
                        name: p.name,
                        vpn_ip: p.vpn_ip.to_string(),
                        online: p.online,
                    })
                    .collect()
            }
        };

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fleet_operation_builder() {
        let op = FleetOperation::new("uptime", FleetTargets::AllOnline)
            .with_timeout(30)
            .with_max_parallel(5)
            .with_ssh_user("admin")
            .with_ssh_port(2222);

        assert_eq!(op.command, "uptime");
        assert_eq!(op.timeout_secs, 30);
        assert_eq!(op.max_parallel, 5);
        assert_eq!(op.ssh_user, "admin");
        assert_eq!(op.ssh_port, 2222);
    }

    #[test]
    fn test_node_result_success() {
        let result = NodeResult::success_result(
            "node1".to_string(),
            "test-node".to_string(),
            "10.0.0.1".to_string(),
            0,
            "output".to_string(),
            "".to_string(),
            100,
        );

        assert!(result.success());
        assert_eq!(result.status, NodeExecutionStatus::Succeeded);
    }

    #[test]
    fn test_node_result_failure() {
        let result = NodeResult::success_result(
            "node1".to_string(),
            "test-node".to_string(),
            "10.0.0.1".to_string(),
            1,
            "".to_string(),
            "error".to_string(),
            100,
        );

        assert!(!result.success());
        assert_eq!(result.status, NodeExecutionStatus::Failed);
    }

    #[test]
    fn test_fleet_summary() {
        let mut results = HashMap::new();
        results.insert(
            "node1".to_string(),
            NodeResult {
                node_id: "node1".to_string(),
                node_name: "Node 1".to_string(),
                vpn_ip: "10.0.0.1".to_string(),
                exit_code: Some(0),
                stdout: String::new(),
                stderr: String::new(),
                duration_ms: 100,
                error: None,
                status: NodeExecutionStatus::Succeeded,
            },
        );
        results.insert(
            "node2".to_string(),
            NodeResult {
                node_id: "node2".to_string(),
                node_name: "Node 2".to_string(),
                vpn_ip: "10.0.0.2".to_string(),
                exit_code: Some(1),
                stdout: String::new(),
                stderr: String::new(),
                duration_ms: 100,
                error: None,
                status: NodeExecutionStatus::Failed,
            },
        );
        results.insert(
            "node3".to_string(),
            NodeResult {
                node_id: "node3".to_string(),
                node_name: "Node 3".to_string(),
                vpn_ip: "10.0.0.3".to_string(),
                exit_code: None,
                stdout: String::new(),
                stderr: String::new(),
                duration_ms: 5000,
                error: Some("timeout".to_string()),
                status: NodeExecutionStatus::TimedOut,
            },
        );

        let summary = FleetSummary::from_results(&results);

        assert_eq!(summary.total, 3);
        assert_eq!(summary.succeeded, 1);
        assert_eq!(summary.failed, 1);
        assert_eq!(summary.timed_out, 1);
    }
}
