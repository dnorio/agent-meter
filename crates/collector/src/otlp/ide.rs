struct IdeRule {
    ide: &'static str,
    ua_patterns: &'static [&'static str],
    svc_patterns: &'static [&'static str],
}

const IDE_RULES: &[IdeRule] = &[
    // Keep copilot-cli first to avoid being swallowed by generic "copilot".
    IdeRule {
        ide: "copilot-cli",
        ua_patterns: &["copilot-cli", "copilot_cli"],
        svc_patterns: &["copilot-cli", "copilot_cli"],
    },
    IdeRule {
        ide: "cursor",
        ua_patterns: &["cursor"],
        svc_patterns: &["cursor"],
    },
    IdeRule {
        ide: "antigravity",
        ua_patterns: &["antigravity"],
        svc_patterns: &["antigravity"],
    },
    IdeRule {
        ide: "claude-code",
        ua_patterns: &["claude-code", "claude_code"],
        svc_patterns: &["claude-code", "claude_code", "claude"],
    },
    IdeRule {
        ide: "codex",
        ua_patterns: &["codex"],
        svc_patterns: &["codex", "openai-codex"],
    },
    IdeRule {
        ide: "opencode",
        ua_patterns: &["opencode"],
        svc_patterns: &["opencode"],
    },
    IdeRule {
        ide: "rust-rover",
        ua_patterns: &["rust-rover", "rustrover"],
        svc_patterns: &["rust-rover", "rustrover"],
    },
    IdeRule {
        ide: "copilot-eclipse",
        ua_patterns: &["eclipse", "jdt"],
        svc_patterns: &["eclipse"],
    },
    IdeRule {
        ide: "copilot-vscode",
        ua_patterns: &["vscode"],
        svc_patterns: &["copilot", "vscode"],
    },
];

fn contains_any(haystack: &str, patterns: &[&str]) -> bool {
    patterns.iter().any(|p| haystack.contains(p))
}

pub fn infer_ide(user_agent: Option<&str>, service_name: Option<&str>) -> Option<String> {
    let ua = user_agent.unwrap_or("").to_lowercase();
    let svc = service_name.unwrap_or("").to_lowercase();

    for rule in IDE_RULES {
        if contains_any(&ua, rule.ua_patterns) || contains_any(&svc, rule.svc_patterns) {
            return Some(rule.ide.to_string());
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::infer_ide;

    #[test]
    fn detects_copilot_vscode_from_ua() {
        assert_eq!(
            infer_ide(Some("Mozilla/5.0 (VSCode)"), None),
            Some("copilot-vscode".to_string())
        );
    }

    #[test]
    fn detects_copilot_vscode_from_service() {
        assert_eq!(
            infer_ide(None, Some("copilot")),
            Some("copilot-vscode".to_string())
        );
    }

    #[test]
    fn detects_copilot_cli_from_ua() {
        assert_eq!(
            infer_ide(Some("github-copilot-cli/1.0"), None),
            Some("copilot-cli".to_string())
        );
    }

    #[test]
    fn detects_copilot_cli_from_service() {
        assert_eq!(
            infer_ide(None, Some("copilot_cli")),
            Some("copilot-cli".to_string())
        );
    }

    #[test]
    fn copilot_cli_has_priority_over_generic_copilot() {
        assert_eq!(
            infer_ide(None, Some("copilot-cli")),
            Some("copilot-cli".to_string())
        );
    }

    #[test]
    fn detects_cursor() {
        assert_eq!(infer_ide(Some("cursor"), None), Some("cursor".to_string()));
    }

    #[test]
    fn detects_antigravity() {
        assert_eq!(
            infer_ide(None, Some("antigravity")),
            Some("antigravity".to_string())
        );
    }

    #[test]
    fn detects_claude_code_from_service_claude() {
        assert_eq!(
            infer_ide(None, Some("claude")),
            Some("claude-code".to_string())
        );
    }

    #[test]
    fn detects_claude_code_from_ua() {
        assert_eq!(
            infer_ide(Some("claude-code/1.2.3"), None),
            Some("claude-code".to_string())
        );
    }

    #[test]
    fn detects_codex() {
        assert_eq!(infer_ide(None, Some("codex")), Some("codex".to_string()));
    }

    #[test]
    fn detects_openai_codex_alias() {
        assert_eq!(
            infer_ide(None, Some("openai-codex")),
            Some("codex".to_string())
        );
    }

    #[test]
    fn detects_opencode() {
        assert_eq!(
            infer_ide(Some("opencode-agent"), None),
            Some("opencode".to_string())
        );
    }

    #[test]
    fn detects_rust_rover() {
        assert_eq!(
            infer_ide(Some("rust-rover"), None),
            Some("rust-rover".to_string())
        );
    }

    #[test]
    fn detects_copilot_eclipse_from_jdt() {
        assert_eq!(
            infer_ide(Some("jdt-language-server"), None),
            Some("copilot-eclipse".to_string())
        );
    }

    #[test]
    fn returns_none_when_no_pattern_matches() {
        assert_eq!(infer_ide(Some("random-agent"), Some("unknown")), None);
    }
}
