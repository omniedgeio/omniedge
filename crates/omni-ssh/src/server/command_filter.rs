//! Command filtering for SSH sessions
//!
//! Provides allowlist/blocklist functionality for controlling which commands
//! can be executed via SSH. This is a key security feature for enterprise deployments.

use crate::types::SshAction;
use regex::Regex;
use std::path::Path;

/// Command filter for enforcing allowed/blocked commands
#[derive(Debug)]
pub struct CommandFilter {
    /// Compiled allowed command patterns
    allowed_patterns: Option<Vec<CompiledPattern>>,
    /// Compiled blocked command patterns (checked first)
    blocked_patterns: Option<Vec<CompiledPattern>>,
    /// Allowed working directories
    allowed_paths: Option<Vec<String>>,
    /// Read-only mode (block write operations)
    read_only: bool,
}

/// A compiled regex pattern with its original string for debugging
#[derive(Debug)]
struct CompiledPattern {
    regex: Regex,
    original: String,
}

/// Result of command filter check
#[derive(Debug, Clone, PartialEq)]
pub enum CommandFilterResult {
    /// Command is allowed
    Allowed,
    /// Command is blocked
    Blocked {
        /// Reason for blocking
        reason: String,
    },
    /// Path is not in the allowed list
    PathNotAllowed {
        /// The path that was rejected
        path: String,
    },
    /// Command violates read-only mode
    ReadOnlyViolation,
}

impl CommandFilter {
    /// Create a new command filter from SSH action configuration
    pub fn new(action: &SshAction) -> anyhow::Result<Self> {
        let allowed_patterns = action.allowed_commands.as_ref().map(|patterns| {
            patterns
                .iter()
                .filter_map(|p| {
                    Regex::new(p).ok().map(|regex| CompiledPattern {
                        regex,
                        original: p.clone(),
                    })
                })
                .collect()
        });

        let blocked_patterns = action.blocked_commands.as_ref().map(|patterns| {
            patterns
                .iter()
                .filter_map(|p| {
                    Regex::new(p).ok().map(|regex| CompiledPattern {
                        regex,
                        original: p.clone(),
                    })
                })
                .collect()
        });

        Ok(Self {
            allowed_patterns,
            blocked_patterns,
            allowed_paths: action.allowed_paths.clone(),
            read_only: action.read_only,
        })
    }

    /// Create a permissive filter that allows everything
    pub fn allow_all() -> Self {
        Self {
            allowed_patterns: None,
            blocked_patterns: None,
            allowed_paths: None,
            read_only: false,
        }
    }

    /// Create a filter that blocks all commands
    pub fn block_all() -> Self {
        Self {
            allowed_patterns: Some(vec![]), // Empty allowlist = block all
            blocked_patterns: None,
            allowed_paths: None,
            read_only: false,
        }
    }

    /// Check if a command is allowed to execute
    pub fn check_command(&self, command: &str) -> CommandFilterResult {
        // 1. Check blocked patterns first (always deny if matched)
        if let Some(ref blocked) = self.blocked_patterns {
            for pattern in blocked {
                if pattern.regex.is_match(command) {
                    tracing::warn!(
                        command = %command,
                        pattern = %pattern.original,
                        "Command blocked by filter"
                    );
                    return CommandFilterResult::Blocked {
                        reason: format!("Command matches blocked pattern: {}", pattern.original),
                    };
                }
            }
        }

        // 2. Check allowed patterns (if set, command must match at least one)
        if let Some(ref allowed) = self.allowed_patterns {
            let matches_any = allowed.iter().any(|p| p.regex.is_match(command));
            if !matches_any {
                tracing::warn!(
                    command = %command,
                    "Command not in allowed list"
                );
                return CommandFilterResult::Blocked {
                    reason: "Command not in allowed list".to_string(),
                };
            }
        }

        // 3. Check for write operations in read-only mode
        if self.read_only && self.is_write_command(command) {
            tracing::warn!(
                command = %command,
                "Command blocked due to read-only mode"
            );
            return CommandFilterResult::ReadOnlyViolation;
        }

        CommandFilterResult::Allowed
    }

    /// Check if working directory is allowed
    pub fn check_path(&self, path: &Path) -> CommandFilterResult {
        if let Some(ref allowed_paths) = self.allowed_paths {
            let path_str = path.to_string_lossy();
            let is_allowed = allowed_paths.iter().any(|allowed| {
                // Check if path starts with allowed prefix
                // Also handle trailing slashes
                let allowed_normalized = allowed.trim_end_matches('/');
                path_str.starts_with(allowed_normalized)
                    || path_str.starts_with(&format!("{}/", allowed_normalized))
                    || path_str == allowed_normalized
            });

            if !is_allowed {
                tracing::warn!(
                    path = %path_str,
                    allowed = ?allowed_paths,
                    "Path not in allowed list"
                );
                return CommandFilterResult::PathNotAllowed {
                    path: path_str.to_string(),
                };
            }
        }

        CommandFilterResult::Allowed
    }

    /// Heuristic check for write operations
    fn is_write_command(&self, command: &str) -> bool {
        let write_indicators = [
            // File deletion
            "rm ",
            "rm\t",
            "rmdir",
            "unlink",
            // Redirection (write to file)
            "> ",
            ">> ",
            // File operations
            "mv ",
            "mv\t",
            "cp ",
            "cp\t",
            "touch ",
            "mkdir",
            // Permissions
            "chmod",
            "chown",
            "chgrp",
            // Editors
            "nano ",
            "vim ",
            "vi ",
            "emacs ",
            "ed ",
            // Write utilities
            "echo ", // when used with >
            "tee ",
            "dd ",
            "install ",
            "rsync ",
            // Package managers
            "apt ",
            "apt-get ",
            "yum ",
            "dnf ",
            "pacman ",
            "brew ",
            "pip ",
            "npm ",
            "cargo ",
            // System modification
            "systemctl start",
            "systemctl stop",
            "systemctl restart",
            "systemctl enable",
            "systemctl disable",
            "service ",
            // Disk operations
            "mkfs",
            "fdisk",
            "parted",
            "format",
            "mount",
            "umount",
        ];

        let cmd_lower = command.to_lowercase();
        write_indicators.iter().any(|ind| cmd_lower.contains(ind))
    }

    /// Check if read-only mode is enabled
    pub fn is_read_only(&self) -> bool {
        self.read_only
    }

    /// Add a blocked pattern at runtime
    pub fn add_blocked_pattern(&mut self, pattern: &str) -> anyhow::Result<()> {
        let regex = Regex::new(pattern)?;
        let compiled = CompiledPattern {
            regex,
            original: pattern.to_string(),
        };

        match self.blocked_patterns.as_mut() {
            Some(patterns) => patterns.push(compiled),
            None => self.blocked_patterns = Some(vec![compiled]),
        }

        Ok(())
    }

    /// Add an allowed pattern at runtime
    pub fn add_allowed_pattern(&mut self, pattern: &str) -> anyhow::Result<()> {
        let regex = Regex::new(pattern)?;
        let compiled = CompiledPattern {
            regex,
            original: pattern.to_string(),
        };

        match self.allowed_patterns.as_mut() {
            Some(patterns) => patterns.push(compiled),
            None => self.allowed_patterns = Some(vec![compiled]),
        }

        Ok(())
    }

    /// Add an allowed path at runtime
    pub fn add_allowed_path(&mut self, path: &str) {
        match self.allowed_paths.as_mut() {
            Some(paths) => paths.push(path.to_string()),
            None => self.allowed_paths = Some(vec![path.to_string()]),
        }
    }
}

impl Default for CommandFilter {
    fn default() -> Self {
        // Default filter: allow all commands but block dangerous ones
        Self {
            allowed_patterns: None,
            blocked_patterns: Some(vec![
                CompiledPattern {
                    regex: Regex::new(r"^rm\s+-rf\s*/").unwrap(),
                    original: r"^rm\s+-rf\s*/".to_string(),
                },
                CompiledPattern {
                    regex: Regex::new(r"^rm\s+.*-rf\s*/").unwrap(),
                    original: r"^rm\s+.*-rf\s*/".to_string(),
                },
                CompiledPattern {
                    regex: Regex::new(r"^shutdown").unwrap(),
                    original: r"^shutdown".to_string(),
                },
                CompiledPattern {
                    regex: Regex::new(r"^reboot").unwrap(),
                    original: r"^reboot".to_string(),
                },
                CompiledPattern {
                    regex: Regex::new(r"^halt").unwrap(),
                    original: r"^halt".to_string(),
                },
                CompiledPattern {
                    regex: Regex::new(r"^poweroff").unwrap(),
                    original: r"^poweroff".to_string(),
                },
                CompiledPattern {
                    regex: Regex::new(r"^mkfs").unwrap(),
                    original: r"^mkfs".to_string(),
                },
                CompiledPattern {
                    regex: Regex::new(r"^dd\s+if=").unwrap(),
                    original: r"^dd\s+if=".to_string(),
                },
            ]),
            allowed_paths: None,
            read_only: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_filter_blocks_dangerous() {
        let filter = CommandFilter::default();

        // These should be blocked
        assert!(matches!(
            filter.check_command("rm -rf /"),
            CommandFilterResult::Blocked { .. }
        ));
        assert!(matches!(
            filter.check_command("shutdown now"),
            CommandFilterResult::Blocked { .. }
        ));
        assert!(matches!(
            filter.check_command("reboot"),
            CommandFilterResult::Blocked { .. }
        ));
        assert!(matches!(
            filter.check_command("mkfs.ext4 /dev/sda"),
            CommandFilterResult::Blocked { .. }
        ));

        // These should be allowed
        assert_eq!(filter.check_command("ls -la"), CommandFilterResult::Allowed);
        assert_eq!(
            filter.check_command("cat file.txt"),
            CommandFilterResult::Allowed
        );
        assert_eq!(
            filter.check_command("grep pattern file"),
            CommandFilterResult::Allowed
        );
    }

    #[test]
    fn test_allowlist_only() {
        let action = SshAction {
            allowed_commands: Some(vec![
                r"^ls".to_string(),
                r"^cat".to_string(),
                r"^grep".to_string(),
            ]),
            blocked_commands: None,
            ..Default::default()
        };

        let filter = CommandFilter::new(&action).unwrap();

        assert_eq!(filter.check_command("ls -la"), CommandFilterResult::Allowed);
        assert_eq!(
            filter.check_command("cat file.txt"),
            CommandFilterResult::Allowed
        );
        assert!(matches!(
            filter.check_command("rm file.txt"),
            CommandFilterResult::Blocked { .. }
        ));
    }

    #[test]
    fn test_read_only_mode() {
        let action = SshAction {
            read_only: true,
            ..Default::default()
        };

        let filter = CommandFilter::new(&action).unwrap();

        assert_eq!(filter.check_command("ls -la"), CommandFilterResult::Allowed);
        assert_eq!(
            filter.check_command("cat file.txt"),
            CommandFilterResult::Allowed
        );
        assert_eq!(
            filter.check_command("rm file.txt"),
            CommandFilterResult::ReadOnlyViolation
        );
        assert_eq!(
            filter.check_command("touch newfile"),
            CommandFilterResult::ReadOnlyViolation
        );
    }

    #[test]
    fn test_path_restriction() {
        let action = SshAction {
            allowed_paths: Some(vec!["/home/user".to_string(), "/tmp".to_string()]),
            ..Default::default()
        };

        let filter = CommandFilter::new(&action).unwrap();

        assert_eq!(
            filter.check_path(Path::new("/home/user/documents")),
            CommandFilterResult::Allowed
        );
        assert_eq!(
            filter.check_path(Path::new("/tmp/test")),
            CommandFilterResult::Allowed
        );
        assert!(matches!(
            filter.check_path(Path::new("/etc/passwd")),
            CommandFilterResult::PathNotAllowed { .. }
        ));
    }

    #[test]
    fn test_block_all() {
        let filter = CommandFilter::block_all();

        assert!(matches!(
            filter.check_command("ls"),
            CommandFilterResult::Blocked { .. }
        ));
        assert!(matches!(
            filter.check_command("anything"),
            CommandFilterResult::Blocked { .. }
        ));
    }

    #[test]
    fn test_allow_all() {
        let filter = CommandFilter::allow_all();

        assert_eq!(
            filter.check_command("rm -rf /"),
            CommandFilterResult::Allowed
        );
        assert_eq!(
            filter.check_command("anything"),
            CommandFilterResult::Allowed
        );
    }
}
