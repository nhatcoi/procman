use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::engine::config::Config;
use crate::engine::logs::read_tail;
use crate::engine::metrics::ProcessMetrics;
use crate::engine::paths::global_data_root;
use crate::engine::supervisor::{self, log_file_for, GlobalProcRow, StatusRow};

pub const UI_REFRESH_INTERVAL: Duration = Duration::from_millis(500);
pub const NORMAL_LOG_LINES: usize = 18;
pub const FULLSCREEN_LOG_LINES: usize = 500;

pub struct AppState {
    pub is_global_mode: bool,
    pub selected_idx: usize,
    pub status_msg: String,
    pub show_qr_modal: bool,
    pub is_fullscreen_log: bool,
    pub is_search_input: bool,
    pub search_query: String,
    pub scroll_offset: usize,
    pub local_rows: Vec<StatusRow>,
    pub global_rows: Vec<GlobalProcRow>,
    pub pending_actions: Arc<Mutex<HashMap<String, String>>>,
    pub metrics: ProcessMetrics,
    pub last_refresh: Instant,
    pub local_config: Option<(PathBuf, Config)>,
    pub display_title: String,
    pub selected_url: Option<String>,
    pub target_log: PathBuf,
    pub row_count: usize,
    pub log_lines: Vec<String>,
}

impl AppState {
    pub fn new(start_all: bool, local_config: Option<(PathBuf, Config)>) -> Self {
        let has_local_config = local_config.is_some();
        Self {
            is_global_mode: start_all || !has_local_config,
            selected_idx: 0,
            status_msg: "ready".to_string(),
            show_qr_modal: false,
            is_fullscreen_log: false,
            is_search_input: false,
            search_query: String::new(),
            scroll_offset: 0,
            local_rows: Vec::new(),
            global_rows: Vec::new(),
            pending_actions: Arc::new(Mutex::new(HashMap::new())),
            metrics: ProcessMetrics::new(),
            last_refresh: Instant::now() - UI_REFRESH_INTERVAL,
            local_config,
            display_title: "none".to_string(),
            selected_url: None,
            target_log: PathBuf::new(),
            row_count: 0,
            log_lines: Vec::new(),
        }
    }

    pub fn needs_refresh(&self) -> bool {
        self.last_refresh.elapsed() >= UI_REFRESH_INTERVAL
    }

    pub fn refresh_rows(&mut self) {
        self.metrics.refresh();
        if let Some((ref cp, ref cfg)) = self.local_config {
            self.local_rows = supervisor::status_with_metrics(cp, cfg, None, &mut self.metrics)
                .unwrap_or_default();
        }
        self.global_rows =
            supervisor::scan_global_processes_with_metrics(&mut self.metrics).unwrap_or_default();

        if let Ok(mut pending) = self.pending_actions.lock() {
            pending.retain(|key, action| {
                if action == "starting..." {
                    let is_up = if let Some(slash_idx) = key.find('/') {
                        let (p_key, s_name) = key.split_at(slash_idx);
                        let s_name = &s_name[1..];
                        self.global_rows.iter().any(|r| {
                            r.project_key == p_key && r.service_name == s_name && r.running
                        })
                    } else {
                        self.local_rows.iter().any(|r| r.name == *key && r.running)
                    };
                    !is_up
                } else if action == "stopping..." {
                    let is_down = if let Some(slash_idx) = key.find('/') {
                        let (p_key, s_name) = key.split_at(slash_idx);
                        let s_name = &s_name[1..];
                        self.global_rows.iter().any(|r| {
                            r.project_key == p_key && r.service_name == s_name && !r.running
                        })
                    } else {
                        self.local_rows.iter().any(|r| r.name == *key && !r.running)
                    };
                    !is_down
                } else {
                    false
                }
            });
        }

        self.last_refresh = Instant::now();
    }

    pub fn clamp_selected_idx(&mut self) {
        self.row_count = if self.is_global_mode {
            self.global_rows.len()
        } else {
            self.local_rows.len()
        };

        if self.row_count == 0 {
            self.selected_idx = 0;
        } else if self.selected_idx >= self.row_count {
            self.selected_idx = self.row_count - 1;
        }
    }

    pub fn refresh_log_view(&mut self) {
        if self.is_global_mode {
            if let Some(g) = self.global_rows.get(self.selected_idx) {
                self.display_title = format!("{}/{}", g.project_key, g.service_name);
                self.target_log = global_data_root()
                    .join(&g.project_key)
                    .join("logs")
                    .join(format!("{}.log", g.service_name));
                self.selected_url = g
                    .tunnel_url
                    .clone()
                    .or_else(|| g.port.map(|p| format!("http://localhost:{}", p)));
            } else {
                self.display_title = "none".to_string();
                self.target_log = PathBuf::new();
                self.selected_url = None;
            }
        } else if let Some((ref cp, _)) = self.local_config {
            if let Some(r) = self.local_rows.get(self.selected_idx) {
                self.display_title = r.name.clone();
                self.target_log = log_file_for(cp, &r.name).unwrap_or_default();
                self.selected_url = r
                    .tunnel_url
                    .clone()
                    .or_else(|| r.port.map(|p| format!("http://localhost:{}", p)));
            } else {
                self.display_title = "none".to_string();
                self.target_log = PathBuf::new();
                self.selected_url = None;
            }
        }

        let lines_to_read = if self.is_fullscreen_log {
            FULLSCREEN_LOG_LINES
        } else {
            NORMAL_LOG_LINES
        };
        let raw_tail = if self.target_log.is_file() {
            read_tail(&self.target_log, lines_to_read)
        } else {
            "(no logs recorded yet)".to_string()
        };

        self.log_lines = raw_tail.lines().map(|s| s.to_string()).collect();
    }
}
