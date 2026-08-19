use anyhow::{Context, Result};
use notify::{
    Config as NotifyConfig, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher,
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::channel;
use std::thread;
use std::time::{Duration, Instant};

use super::config::Config;
use super::supervisor;

const DEFAULT_DEBOUNCE_DURATION: Duration = Duration::from_millis(350);

const DEFAULT_IGNORE_PATTERNS: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    "dist",
    "build",
    ".next",
    ".nuxt",
    ".cache",
    ".idea",
    ".vscode",
    "logs",
    ".procman",
    "state.json",
    "registry.json",
    "update_cache.json",
];

pub fn should_ignore_path(path: &Path, custom_ignores: &[String]) -> bool {
    let path_str = path.to_string_lossy();

    // Ignore temporary / editor swap files
    if path_str.ends_with('~') || path_str.ends_with(".tmp") || path_str.ends_with(".log") {
        return true;
    }

    // Check default ignored directory/file patterns
    for component in path.components() {
        let comp_str = component.as_os_str().to_string_lossy();
        for pattern in DEFAULT_IGNORE_PATTERNS {
            if comp_str == *pattern {
                return true;
            }
        }
    }

    // Check custom ignore patterns
    for custom in custom_ignores {
        if path_str.contains(custom) {
            return true;
        }
    }

    false
}

fn resolve_watch_path(project_dir: &Path, rel_or_abs: &str) -> PathBuf {
    let p = Path::new(rel_or_abs);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        project_dir.join(p)
    }
}

pub fn watch_and_reload(
    config_path: &Path,
    config: &Config,
    target_names: Option<Vec<String>>,
) -> Result<()> {
    let project_dir = config_path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| config_path.to_path_buf());

    let (tx, rx) = channel::<notify::Result<Event>>();
    let mut watcher = RecommendedWatcher::new(tx, NotifyConfig::default())
        .context("Failed to initialize file watcher")?;

    let targets_to_watch: Vec<String> = if let Some(targets) = target_names {
        targets
    } else {
        // Collect all services that have watch enabled, or all if none explicitly enabled
        let explicit_watch: Vec<String> = config
            .processes
            .iter()
            .filter(|(_, def)| def.is_watch_enabled())
            .map(|(name, _)| name.clone())
            .collect();

        if explicit_watch.is_empty() {
            config.processes.keys().cloned().collect()
        } else {
            explicit_watch
        }
    };

    if targets_to_watch.is_empty() {
        println!("No services to watch.");
        return Ok(());
    }

    // Map watch path -> service names
    let mut path_to_services: HashMap<PathBuf, Vec<String>> = HashMap::new();

    for service_name in &targets_to_watch {
        if let Some(def) = config.processes.get(service_name) {
            let rel_paths = def.get_watch_paths();
            let base_dir = def
                .cwd
                .as_deref()
                .map(|c| resolve_watch_path(&project_dir, c))
                .unwrap_or_else(|| project_dir.clone());

            for p in rel_paths {
                let full_path = if p == "." {
                    base_dir.clone()
                } else {
                    resolve_watch_path(&base_dir, &p)
                };

                if full_path.exists() {
                    path_to_services
                        .entry(full_path.clone())
                        .or_default()
                        .push(service_name.clone());
                }
            }
        }
    }

    // Register watch paths
    println!("👀 Starting procman live watcher for:");
    for (path, services) in &path_to_services {
        println!(
            "   - {:?} -> [{}]",
            path.strip_prefix(&project_dir).unwrap_or(path),
            services.join(", ")
        );
        watcher
            .watch(path, RecursiveMode::Recursive)
            .with_context(|| format!("Failed to watch path {:?}", path))?;
    }
    println!("   Press Ctrl+C to stop watching.\n");

    let mut last_restart_times: HashMap<String, Instant> = HashMap::new();

    loop {
        match rx.recv() {
            Ok(Ok(event)) => {
                match event.kind {
                    EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_) => {}
                    _ => continue,
                }

                // Filter out noise files
                let relevant_paths: Vec<_> = event
                    .paths
                    .iter()
                    .filter(|p| {
                        !should_ignore_path(
                            p,
                            &config
                                .processes
                                .values()
                                .flat_map(|d| d.watch_ignore.clone())
                                .collect::<Vec<_>>(),
                        )
                    })
                    .collect();

                if relevant_paths.is_empty() {
                    continue;
                }

                let trigger_path = relevant_paths[0];
                let now = Instant::now();

                // Identify which services should restart
                for (watch_root, services) in &path_to_services {
                    if trigger_path.starts_with(watch_root) {
                        for service_name in services {
                            let last_restart = last_restart_times
                                .get(service_name)
                                .copied()
                                .unwrap_or_else(|| now - DEFAULT_DEBOUNCE_DURATION * 2);

                            if now.duration_since(last_restart) >= DEFAULT_DEBOUNCE_DURATION {
                                last_restart_times.insert(service_name.clone(), now);

                                let rel_display = trigger_path
                                    .strip_prefix(&project_dir)
                                    .unwrap_or(trigger_path)
                                    .display();

                                println!(
                                    "🔄 [watch: {}] File change detected: {}. Restarting...",
                                    service_name, rel_display
                                );

                                if let Err(e) = supervisor::restart(
                                    config_path,
                                    config,
                                    Some(service_name),
                                    false,
                                ) {
                                    eprintln!("❌ Failed to restart [{}]: {}", service_name, e);
                                }
                            }
                        }
                    }
                }
            }
            Ok(Err(e)) => {
                eprintln!("⚠️  Watcher error: {:?}", e);
            }
            Err(_) => {
                break;
            }
        }
        thread::sleep(Duration::from_millis(50));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_ignore_path() {
        assert!(should_ignore_path(Path::new("/app/.git/HEAD"), &[]));
        assert!(should_ignore_path(
            Path::new("/app/node_modules/express/index.js"),
            &[]
        ));
        assert!(should_ignore_path(
            Path::new("/app/target/debug/procman"),
            &[]
        ));
        assert!(should_ignore_path(Path::new("/app/logs/api.log"), &[]));
        assert!(should_ignore_path(Path::new("/app/src/temp.tmp"), &[]));
        assert!(should_ignore_path(Path::new("/app/src/state.json"), &[]));

        assert!(!should_ignore_path(Path::new("/app/src/main.rs"), &[]));
        assert!(!should_ignore_path(Path::new("/app/web/src/App.tsx"), &[]));
        assert!(!should_ignore_path(Path::new("/app/Cargo.toml"), &[]));

        assert!(should_ignore_path(
            Path::new("/app/data/test.sqlite"),
            &["test.sqlite".to_string()]
        ));
    }
}
