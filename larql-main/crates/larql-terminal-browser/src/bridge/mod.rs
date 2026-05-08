// Copyright 2025 LARQL Project
// SPDX-License-Identifier: Apache-2.0

//! Bridge layer coordinating between Ratatui and Carbonyl.

pub mod coordinator;

pub use coordinator::RenderCoordinator;

use crate::ratatui::{Capability, Task, WorkspaceInfo};
use reqwest::Client;
use serde::Deserialize;
use std::path::PathBuf;
use std::time::Instant;

/// Rendering mode for the hybrid TUI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderMode {
    /// Ratatui navigation mode (layout, navigation, status).
    Navigation,
    /// Carbonyl content mode (rich HTML/CSS/JS rendering).
    Content,
}

/// Bridge state managing the coordination between Ratatui and Carbonyl.
#[derive(Debug, Clone)]
pub struct BridgeState {
    /// Current render mode.
    pub mode: RenderMode,
    /// Current active task.
    pub active_task: Task,
    /// Workbench URL.
    pub workbench_url: String,
    /// Carbonyl binary path.
    pub carbonyl_path: Option<PathBuf>,
    /// Carbonyl process (if running).
    pub carbonyl_process: Option<CarbonylProcess>,
    /// Last mode switch time.
    pub last_mode_switch: Instant,
    /// Performance metrics.
    pub performance: BridgePerformance,
    /// HTTP client for backend API calls.
    pub http_client: Client,
}

#[derive(Debug, Clone, Copy)]
pub struct CarbonylProcess {
    pub pid: u32,
    pub suspended: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct BridgePerformance {
    pub mode_switch_time_ms: u64,
    pub carbonyl_startup_time_ms: u64,
    pub carbonyl_suspend_time_ms: u64,
    pub carbonyl_resume_time_ms: u64,
}

impl Default for BridgeState {
    fn default() -> Self {
        Self {
            mode: RenderMode::Navigation,
            active_task: Task::Workspace,
            workbench_url: String::new(),
            carbonyl_path: None,
            carbonyl_process: None,
            last_mode_switch: Instant::now(),
            performance: BridgePerformance {
                mode_switch_time_ms: 0,
                carbonyl_startup_time_ms: 0,
                carbonyl_suspend_time_ms: 0,
                carbonyl_resume_time_ms: 0,
            },
            http_client: Client::new(),
        }
    }
}

impl BridgeState {
    /// Switch to a different render mode.
    pub fn switch_mode(&mut self, new_mode: RenderMode) {
        let start = Instant::now();
        self.mode = new_mode;
        self.last_mode_switch = Instant::now();
        self.performance.mode_switch_time_ms = start.elapsed().as_millis() as u64;
    }

    /// Switch to a different task.
    pub fn switch_task(&mut self, task: Task) {
        self.active_task = task;
    }

    /// Get the URL for the current task.
    pub fn task_url(&self) -> String {
        let path = match self.active_task {
            Task::Workspace => "/workspace",
            Task::Studio => "/studio",
            Task::Explorer => "/explorer",
            Task::LQL => "/lql",
            Task::Trace => "/trace",
            Task::Recipes => "/recipes",
            Task::Runs => "/runs",
        };
        format!("{}{}", self.workbench_url.trim_end_matches('/'), path)
    }

    /// Check if Carbonyl process is running.
    pub fn is_carbonyl_running(&self) -> bool {
        self.carbonyl_process.is_some()
    }

    /// Check if Carbonyl process is suspended.
    pub fn is_carbonyl_suspended(&self) -> bool {
        self.carbonyl_process
            .as_ref()
            .map(|p| p.suspended)
            .unwrap_or(false)
    }

    /// Fetch workspace info from backend API.
    pub async fn fetch_workspace_info(
        &self,
    ) -> Result<Option<WorkspaceInfo>, Box<dyn std::error::Error>> {
        let url = format!(
            "{}/api/workspace/current",
            self.workbench_url.trim_end_matches('/')
        );

        let response = self.http_client.get(&url).send().await?;

        if !response.status().is_success() {
            return Ok(None);
        }

        let api_response: WorkspaceApiResponse = response.json().await?;

        Ok(Some(WorkspaceInfo {
            name: api_response.workspace.display_name,
            extract_level: api_response
                .workspace
                .extract_level
                .unwrap_or_else(|| "unknown".to_string()),
        }))
    }

    /// Fetch capabilities from workspace info.
    pub async fn fetch_capabilities(
        &self,
        _workspace_info: &WorkspaceInfo,
    ) -> Result<Vec<Capability>, Box<dyn std::error::Error>> {
        let url = format!(
            "{}/api/workspace/current",
            self.workbench_url.trim_end_matches('/')
        );

        let response = self.http_client.get(&url).send().await?;

        if !response.status().is_success() {
            return Ok(vec![]);
        }

        let api_response: WorkspaceApiResponse = response.json().await?;
        let mut capabilities = Vec::new();

        if api_response.workspace.supports_infer {
            capabilities.push(Capability::Infer);
        }
        if api_response.workspace.supports_trace {
            capabilities.push(Capability::Trace);
        }
        if api_response.workspace.supports_mlx {
            capabilities.push(Capability::Mlx);
        }
        if api_response.workspace.has_relation_labels {
            capabilities.push(Capability::Labels);
        }
        capabilities.push(Capability::Browse);
        if api_response.workspace.has_model_weights {
            capabilities.push(Capability::WalkFfn);
        }

        Ok(capabilities)
    }
}

/// API response from /api/workspace/current.
#[derive(Debug, Deserialize)]
struct WorkspaceApiResponse {
    workspace: WorkspaceData,
}

/// Workspace data from API.
#[derive(Debug, Deserialize)]
struct WorkspaceData {
    display_name: String,
    extract_level: Option<String>,
    supports_infer: bool,
    supports_trace: bool,
    supports_mlx: bool,
    has_relation_labels: bool,
    has_model_weights: bool,
}
