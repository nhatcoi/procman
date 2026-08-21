use super::rules::{DiagnosticMatch, DiagnosticRule, RuleCategory};

#[allow(dead_code)]
pub fn get_diagnostic_rules() -> &'static [DiagnosticRule] {
    &RULES
}


pub fn evaluate_rules(log_text: &str, exit_code: Option<i32>) -> Option<DiagnosticMatch> {
    // 1. Check exit code specific heuristics first (e.g. exit code 137 / SIGKILL is Linux OOM Killer)
    if let Some(code) = exit_code {
        if code == 137 || code == -9 {
            return Some(DiagnosticMatch {
                rule_id: "oom_sigkill_137".to_string(),
                rule_name: "Killed by Linux Out-Of-Memory (OOM) Killer".to_string(),
                category: RuleCategory::OutOfMemory,
                explanation: "The process was forcefully terminated (SIGKILL / Exit 137) because system or container RAM was exhausted.".to_string(),
                fix_command: Some("procman doctor --ai".to_string()),
                matched_line: format!("Process exited with status code {}", code),
            });
        }
        if code == 127 {
            return Some(DiagnosticMatch {
                rule_id: "exit_127_command_not_found".to_string(),
                rule_name: "Command / Binary Not Found (Exit 127)".to_string(),
                category: RuleCategory::MissingBinary,
                explanation: "The specified command or executable does not exist in system $PATH.".to_string(),
                fix_command: None,
                matched_line: format!("Process exited with status code 127"),
            });
        }
    }

    // 2. Evaluate all signature rules in order
    for rule in RULES.iter() {
        if let Some(m) = rule.evaluate(log_text) {
            return Some(m);
        }
    }

    None
}

static RULES: [DiagnosticRule; 24] = [
    // --- 1. Port Conflicts ---
    DiagnosticRule {
        id: "node_eaddrinuse",
        name: "Node.js Port In Use",
        category: RuleCategory::PortConflict,
        pattern: r"EADDRINUSE.*:(\d+)",
        explanation_template: "Port $1 is already in use by another process or a previous zombie instance.",
        fix_template: Some("procman kill-port $1"),
    },
    DiagnosticRule {
        id: "generic_address_in_use",
        name: "Address Already In Use",
        category: RuleCategory::PortConflict,
        pattern: r"(?:address already in use|bind: address already in use|listen tcp .*:(\d+): bind)",
        explanation_template: "The target network port is already occupied by an active process.",
        fix_template: Some("lsof -i :$1"),
    },
    DiagnosticRule {
        id: "python_address_in_use",
        name: "Python Address Already In Use",
        category: RuleCategory::PortConflict,
        pattern: r"OSError: \[Errno 98\] Address already in use",
        explanation_template: "The socket port configured for this Python server is already bounded.",
        fix_template: Some("procman stop && procman start"),
    },

    // --- 2. Missing Dependencies ---
    DiagnosticRule {
        id: "node_cannot_find_module",
        name: "Node.js Missing Module",
        category: RuleCategory::MissingDependency,
        pattern: r"Cannot find module '([^']+)'",
        explanation_template: "The Node.js package '$1' is required but not installed in node_modules.",
        fix_template: Some("npm install $1"),
    },
    DiagnosticRule {
        id: "node_cannot_find_package",
        name: "Node.js Missing Package",
        category: RuleCategory::MissingDependency,
        pattern: r"ERR_MODULE_NOT_FOUND.*Cannot find package '([^']+)'",
        explanation_template: "Package '$1' cannot be found in project dependencies.",
        fix_template: Some("npm install $1"),
    },
    DiagnosticRule {
        id: "python_module_not_found",
        name: "Python Module Not Found",
        category: RuleCategory::MissingDependency,
        pattern: r"ModuleNotFoundError: No module named '([^']+)'",
        explanation_template: "Python library '$1' is missing from the active virtualenv / python environment.",
        fix_template: Some("pip install $1"),
    },
    DiagnosticRule {
        id: "python_import_error",
        name: "Python Import Error",
        category: RuleCategory::MissingDependency,
        pattern: r"ImportError: cannot import name '([^']+)' from '([^']+)'",
        explanation_template: "Failed to import symbol '$1' from module '$2' (potential version mismatch).",
        fix_template: Some("pip install --upgrade $2"),
    },
    DiagnosticRule {
        id: "go_cannot_find_package",
        name: "Go Package Not Found",
        category: RuleCategory::MissingDependency,
        pattern: r"cannot find package\s+\x22([^\x22]+)\x22",
        explanation_template: "Go package '$1' is not present in go.mod or GOPATH.",
        fix_template: Some("go get $1 && go mod tidy"),
    },

    // --- 3. Missing Executables / Binaries ---
    DiagnosticRule {
        id: "sh_command_not_found",
        name: "Shell Command Not Found",
        category: RuleCategory::MissingBinary,
        pattern: r"(?:sh|bash|zsh): (?:line \d+: |\d+: )?([^:\s]+): (?:command )?not found",
        explanation_template: "Executable '$1' is not installed or not available in the system PATH.",
        fix_template: Some("which $1"),
    },

    DiagnosticRule {
        id: "exec_not_found_path",
        name: "Executable Not Found in PATH",
        category: RuleCategory::MissingBinary,
        pattern: r"exec: \x22([^\x22]+)\x22: executable file not found in \$PATH",
        explanation_template: "Binary '$1' is missing from the environment PATH.",
        fix_template: Some("which $1"),
    },
    DiagnosticRule {
        id: "no_such_file_or_directory",
        name: "No Such File or Directory",
        category: RuleCategory::MissingBinary,
        pattern: r"(?:cannot find|No such file or directory)[: ]+([^\n\r]+)",
        explanation_template: "Target path or file '$1' does not exist.",
        fix_template: None,
    },

    // --- 4. Database Connection Refused ---
    DiagnosticRule {
        id: "postgres_connection_refused",
        name: "PostgreSQL Connection Refused",
        category: RuleCategory::DatabaseConnection,
        pattern: r"(?:ECONNREFUSED.*5432|could not connect to server: Connection refused.*5432|dial tcp .*:5432: connect: connection refused)",
        explanation_template: "PostgreSQL database is offline or not accepting connections on port 5432.",
        fix_template: Some("docker compose up -d || sudo systemctl start postgresql"),
    },
    DiagnosticRule {
        id: "mysql_connection_refused",
        name: "MySQL Connection Refused",
        category: RuleCategory::DatabaseConnection,
        pattern: r"(?:ECONNREFUSED.*3306|Can't connect to MySQL server on .* \(111\)|dial tcp .*:3306: connect: connection refused)",
        explanation_template: "MySQL/MariaDB service is not running on port 3306.",
        fix_template: Some("docker compose up -d || sudo systemctl start mysql"),
    },
    DiagnosticRule {
        id: "redis_connection_refused",
        name: "Redis Connection Refused",
        category: RuleCategory::DatabaseConnection,
        pattern: r"(?:ECONNREFUSED.*6379|Redis connection to .*:6379 failed|dial tcp .*:6379: connect: connection refused)",
        explanation_template: "Redis cache service is down or unreachable on port 6379.",
        fix_template: Some("docker compose up -d redis || sudo systemctl start redis"),
    },
    DiagnosticRule {
        id: "mongodb_connection_refused",
        name: "MongoDB Connection Refused",
        category: RuleCategory::DatabaseConnection,
        pattern: r"(?:ECONNREFUSED.*27017|MongoServerSelectionError: connect ECONNREFUSED)",
        explanation_template: "MongoDB instance is offline or unreachable on port 27017.",
        fix_template: Some("docker compose up -d mongodb || sudo systemctl start mongod"),
    },

    // --- 5. Database Schema & Migration ---
    DiagnosticRule {
        id: "prisma_missing_table",
        name: "Prisma Table Missing",
        category: RuleCategory::DatabaseMigration,
        pattern: r"P2021: The table `?([^`]+)`? does not exist in the current database",
        explanation_template: "Database table '$1' is missing. Database schema migrations have not been applied.",
        fix_template: Some("npx prisma migrate dev"),
    },
    DiagnosticRule {
        id: "sql_relation_does_not_exist",
        name: "Database Relation Does Not Exist",
        category: RuleCategory::DatabaseMigration,
        pattern: r#"relation \x22([^\x22]+)\x22 does not exist"#,
        explanation_template: "Database table/relation '$1' is missing. Pending migrations need to be run.",
        fix_template: Some("npm run migrate || npx prisma migrate dev || alembic upgrade head"),
    },

    // --- 6. Memory Limits & OOM ---
    DiagnosticRule {
        id: "v8_heap_oom",
        name: "Node.js Heap Out of Memory",
        category: RuleCategory::OutOfMemory,
        pattern: r"(?:JavaScript heap out of memory|Allocation failed - JavaScript heap out of memory)",
        explanation_template: "V8 JavaScript heap memory limit was exceeded.",
        fix_template: Some("export NODE_OPTIONS=\"--max-old-space-size=4096\""),
    },
    DiagnosticRule {
        id: "go_runtime_oom",
        name: "Go Runtime Out of Memory",
        category: RuleCategory::OutOfMemory,
        pattern: r"fatal error: runtime: out of memory",
        explanation_template: "Go runtime exhausted all allocatable memory.",
        fix_template: None,
    },

    // --- 7. Permissions ---
    DiagnosticRule {
        id: "permission_denied",
        name: "Permission Denied",
        category: RuleCategory::PermissionDenied,
        pattern: r"(?:EACCES: permission denied|Permission denied \(os error 13\)|permission denied: ([^\n\r]+))",
        explanation_template: "Execution or file write permission was rejected by operating system.",
        fix_template: Some("chmod +x <target_file>"),
    },

    // --- 8. Environment Variables ---
    DiagnosticRule {
        id: "missing_env_var",
        name: "Missing Environment Variable",
        category: RuleCategory::MissingEnvironment,
        pattern: r"(?:missing required environment variable[: ]+([A-Z0-9_]+)|KeyError: '([A-Z0-9_]+)')",
        explanation_template: "Required environment variable '$1' is not defined in .env or procman.yaml.",
        fix_template: None,
    },

    // --- 9. Rust / Generic Panic ---
    DiagnosticRule {
        id: "rust_panicked",
        name: "Rust Runtime Panic",
        category: RuleCategory::SyntaxOrPanic,
        pattern: r"thread '([^']+)' panicked at (?:'([^']+)', )?([^\n\r]+)",
        explanation_template: "Rust thread '$1' panicked at $3.",
        fix_template: Some("cargo check"),
    },
    DiagnosticRule {
        id: "python_unhandled_exception",
        name: "Python Unhandled Exception",
        category: RuleCategory::SyntaxOrPanic,
        pattern: r"Traceback \(most recent call last\):",
        explanation_template: "Python raised an unhandled runtime exception.",
        fix_template: Some("procman doctor --ai"),
    },
    DiagnosticRule {
        id: "go_panic",
        name: "Go Runtime Panic",
        category: RuleCategory::SyntaxOrPanic,
        pattern: r"panic: (.*)",
        explanation_template: "Go application panicked: $1",
        fix_template: Some("procman doctor --ai"),
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_eaddrinuse_detection() {
        let log = "Error: listen EADDRINUSE: address already in use :::8080\n    at Server.setupListenHandle";
        let m = evaluate_rules(log, None).expect("Should detect port conflict");
        assert_eq!(m.category, RuleCategory::PortConflict);
        assert_eq!(m.fix_command.as_deref(), Some("procman kill-port 8080"));
    }

    #[test]
    fn test_postgres_offline_detection() {
        let log = "PrismaClientInitializationError: Can't reach database server at `localhost:5432`\nconnect ECONNREFUSED 127.0.0.1:5432";
        let m = evaluate_rules(log, None).expect("Should detect postgres connection failure");
        assert_eq!(m.category, RuleCategory::DatabaseConnection);
    }

    #[test]
    fn test_exit_137_sigkill() {
        let m = evaluate_rules("", Some(137)).expect("Should detect OOM from exit code 137");
        assert_eq!(m.category, RuleCategory::OutOfMemory);
    }

    #[test]
    fn test_python_missing_module() {
        let log = "Traceback (most recent call last):\n  File \"app.py\", line 1\nModuleNotFoundError: No module named 'fastapi'";
        let m = evaluate_rules(log, None).expect("Should detect python missing module");
        assert_eq!(m.category, RuleCategory::MissingDependency);
        assert_eq!(m.fix_command.as_deref(), Some("pip install fastapi"));
    }

    #[test]
    fn test_shell_command_not_found() {
        let log = "sh: 1: cargo: not found";
        let m = evaluate_rules(log, None).expect("Should detect command not found");
        assert_eq!(m.category, RuleCategory::MissingBinary);
        assert_eq!(m.fix_command.as_deref(), Some("which cargo"));
    }

    #[test]
    fn test_prisma_missing_migration() {
        let log = "Invalid `prisma.user.findMany()` invocation:\nP2021: The table `User` does not exist in the current database";
        let m = evaluate_rules(log, None).expect("Should detect prisma missing migration");
        assert_eq!(m.category, RuleCategory::DatabaseMigration);
        assert_eq!(m.fix_command.as_deref(), Some("npx prisma migrate dev"));
    }

    #[test]
    fn test_permission_denied() {
        let log = "Error: EACCES: permission denied, open '/var/run/app.sock'";
        let m = evaluate_rules(log, None).expect("Should detect permission denied");
        assert_eq!(m.category, RuleCategory::PermissionDenied);
    }

    #[test]
    fn test_missing_environment_variable() {
        let log = "FatalError: missing required environment variable: DATABASE_URL";
        let m = evaluate_rules(log, None).expect("Should detect missing env var");
        assert_eq!(m.category, RuleCategory::MissingEnvironment);
        assert!(m.explanation.contains("DATABASE_URL"));
    }

    #[test]
    fn test_rust_panicked() {
        let log = "thread 'main' panicked at 'called `Option::unwrap()` on a `None` value', src/main.rs:42:10";
        let m = evaluate_rules(log, None).expect("Should detect rust panic");
        assert_eq!(m.category, RuleCategory::SyntaxOrPanic);
        assert_eq!(m.fix_command.as_deref(), Some("cargo check"));
    }
}

