use regex::Regex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleCategory {
    PortConflict,
    MissingDependency,
    MissingBinary,
    DatabaseConnection,
    DatabaseMigration,
    OutOfMemory,
    PermissionDenied,
    MissingEnvironment,
    SyntaxOrPanic,
}

impl RuleCategory {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::PortConflict => "Port Conflict",
            Self::MissingDependency => "Missing Dependency",
            Self::MissingBinary => "Missing Executable",
            Self::DatabaseConnection => "Database Connection",
            Self::DatabaseMigration => "Database Schema / Migration",
            Self::OutOfMemory => "Out of Memory (OOM)",
            Self::PermissionDenied => "Permission Denied",
            Self::MissingEnvironment => "Environment Variable",
            Self::SyntaxOrPanic => "Runtime Panic / Syntax Error",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            Self::PortConflict => "🔌",
            Self::MissingDependency => "📦",
            Self::MissingBinary => "⚙️",
            Self::DatabaseConnection => "🗄️",
            Self::DatabaseMigration => "📐",
            Self::OutOfMemory => "💥",
            Self::PermissionDenied => "🔒",
            Self::MissingEnvironment => "🔑",
            Self::SyntaxOrPanic => "⚠️",
        }
    }
}

#[derive(Debug, Clone)]
pub struct DiagnosticRule {
    pub id: &'static str,
    pub name: &'static str,
    pub category: RuleCategory,
    pub pattern: &'static str,
    pub explanation_template: &'static str,
    pub fix_template: Option<&'static str>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct DiagnosticMatch {
    pub rule_id: String,
    pub rule_name: String,
    pub category: RuleCategory,
    pub explanation: String,
    pub fix_command: Option<String>,
    pub matched_line: String,
}


impl DiagnosticRule {
    pub fn evaluate(&self, text: &str) -> Option<DiagnosticMatch> {
        let Ok(re) = Regex::new(self.pattern) else {
            return None;
        };

        for line in text.lines() {
            if let Some(caps) = re.captures(line) {
                let explanation = substitute_captures(self.explanation_template, &caps);
                let fix_command = self
                    .fix_template
                    .map(|t| substitute_captures(t, &caps));

                return Some(DiagnosticMatch {
                    rule_id: self.id.to_string(),
                    rule_name: self.name.to_string(),
                    category: self.category,
                    explanation,
                    fix_command,
                    matched_line: line.trim().to_string(),
                });
            }
        }
        None
    }
}

fn substitute_captures(template: &str, caps: &regex::Captures) -> String {
    let mut result = template.to_string();
    for i in 1..caps.len() {
        if let Some(m) = caps.get(i) {
            let placeholder = format!("${}", i);
            result = result.replace(&placeholder, m.as_str());
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_substitute_captures() {
        let rule = DiagnosticRule {
            id: "node_eaddrinuse",
            name: "Port in use",
            category: RuleCategory::PortConflict,
            pattern: r"EADDRINUSE.*:(\d+)",
            explanation_template: "Port $1 is occupied by another process.",
            fix_template: Some("procman kill-port $1"),
        };

        let log = "Error: listen EADDRINUSE: address already in use :::3000\n    at Server.setupListenHandle";
        let diag = rule.evaluate(log).expect("Should match rule");
        assert_eq!(diag.category, RuleCategory::PortConflict);
        assert_eq!(diag.explanation, "Port 3000 is occupied by another process.");
        assert_eq!(diag.fix_command.as_deref(), Some("procman kill-port 3000"));
    }

    #[test]
    fn test_missing_module_capture() {
        let rule = DiagnosticRule {
            id: "node_missing_module",
            name: "Missing Node.js Module",
            category: RuleCategory::MissingDependency,
            pattern: r"Cannot find module '([^']+)'",
            explanation_template: "Missing required module '$1'.",
            fix_template: Some("npm install $1"),
        };

        let log = "Error: Cannot find module 'express'\nRequire stack:\n- /app/index.js";
        let diag = rule.evaluate(log).expect("Should match rule");
        assert_eq!(diag.explanation, "Missing required module 'express'.");
        assert_eq!(diag.fix_command.as_deref(), Some("npm install express"));
    }
}
