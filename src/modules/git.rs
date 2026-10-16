use crate::cache::{GitLookup, GitSnapshot};
use crate::core::layout::truncate_plain_to_width;
use crate::core::prompt::PromptContext;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::Duration;
use unicode_width::UnicodeWidthStr;

/// Result of resolving Git state for a render, including fallback states so
/// the segment never silently disappears.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GitResolution {
    /// A usable snapshot. `stale` marks data served from an expired entry
    /// while a background refresh is in flight.
    Ready {
        /// The cached/computed snapshot.
        snapshot: GitSnapshot,
        /// Whether the snapshot came from an expired cache entry.
        stale: bool,
    },
    /// A repository was detected but its state is not available yet.
    Pending,
    /// The directory is not inside a Git repository.
    NotRepository,
}

/// Render the Git segment, emitting an explicit placeholder while a refresh is
/// pending rather than dropping the segment.
pub fn render_with_max_width(context: &PromptContext, max_visible_width: usize) -> String {
    if max_visible_width == 0 {
        return String::new();
    }

    match resolve_for_context(context) {
        GitResolution::NotRepository => String::new(),
        GitResolution::Pending => render_placeholder(context, max_visible_width),
        GitResolution::Ready { snapshot, .. } => {
            render_snapshot(&snapshot, context, max_visible_width)
        }
    }
}

fn render_placeholder(context: &PromptContext, max_visible_width: usize) -> String {
    let placeholder = context.config.git_placeholder();
    if UnicodeWidthStr::width(placeholder) > max_visible_width {
        return String::new();
    }
    format!(
        "\x1b[{}m{placeholder}\x1b[0m",
        context.config.git_branch_color()
    )
}

fn render_snapshot(
    snapshot: &GitSnapshot,
    context: &PromptContext,
    max_visible_width: usize,
) -> String {
    let mut status_tokens = status_tokens(snapshot, context.config.as_ref());
    let branch_icon = context.config.git_branch_icon();
    let icon_with_space = format!("{branch_icon} ");
    let icon_width = UnicodeWidthStr::width(icon_with_space.as_str());
    let status_widths = status_tokens
        .iter()
        .map(|(plain, _)| UnicodeWidthStr::width(plain.as_str()))
        .collect::<Vec<_>>();
    let mut status_total_width = if status_tokens.is_empty() {
        0
    } else {
        status_widths.iter().sum::<usize>() + (status_tokens.len().saturating_sub(1))
    };

    loop {
        let status_width = if status_tokens.is_empty() {
            0
        } else {
            1 + status_total_width
        };
        let branch_budget = max_visible_width
            .saturating_sub(icon_width + status_width)
            .max(1);

        let branch = if UnicodeWidthStr::width(snapshot.branch.as_str()) > branch_budget {
            truncate_plain_to_width(snapshot.branch.as_str(), branch_budget)
        } else {
            snapshot.branch.clone()
        };

        let mut out = format!(
            "\x1b[{}m{} {}\x1b[0m",
            context.config.git_branch_color(),
            branch_icon,
            branch
        );
        if !status_tokens.is_empty() {
            out.push(' ');
            out.push_str(
                &status_tokens
                    .iter()
                    .map(|(_, styled)| styled.as_str())
                    .collect::<Vec<_>>()
                    .join(" "),
            );
        }

        if crate::core::layout::visible_width(out.as_str()) <= max_visible_width {
            return out;
        }

        if status_tokens.is_empty() {
            let plain = crate::core::layout::strip_ansi(out.as_str());
            return format!(
                "\x1b[{}m{}\x1b[0m",
                context.config.git_branch_color(),
                truncate_plain_to_width(plain.as_str(), max_visible_width)
            );
        }

        let _removed = status_tokens.pop();
        if let Some(width) = status_widths.get(status_tokens.len()) {
            status_total_width = status_total_width.saturating_sub(*width).saturating_sub(1);
        } else {
            status_total_width = status_tokens
                .iter()
                .map(|(plain, _)| UnicodeWidthStr::width(plain.as_str()))
                .sum::<usize>()
                .saturating_add(status_tokens.len().saturating_sub(1));
        }
    }
}

fn status_tokens(
    snapshot: &GitSnapshot,
    config: &crate::core::config::Config,
) -> Vec<(String, String)> {
    let mut status = Vec::new();

    if snapshot.staged > 0 {
        let plain = format!("{}{}", config.git_staged_icon(), snapshot.staged);
        let styled = format!("\x1b[{}m{}\x1b[0m", config.git_staged_color(), plain);
        status.push((plain, styled));
    }
    if snapshot.unstaged > 0 {
        let plain = format!("{}{}", config.git_unstaged_icon(), snapshot.unstaged);
        let styled = format!("\x1b[{}m{}\x1b[0m", config.git_unstaged_color(), plain);
        status.push((plain, styled));
    }
    if snapshot.untracked > 0 {
        let plain = format!("{}{}", config.git_untracked_icon(), snapshot.untracked);
        let styled = format!("\x1b[{}m{}\x1b[0m", config.git_untracked_color(), plain);
        status.push((plain, styled));
    }

    status
}

/// Resolve Git state for a prompt context.
///
/// A pre-resolved state (injected by the daemon render path) always wins. In a
/// client process the daemon is queried first, falling back to a bounded local
/// computation so a missing daemon never yields a silently empty segment.
fn resolve_for_context(context: &PromptContext) -> GitResolution {
    if let Some(state) = &context.git_state {
        return state.clone();
    }

    let Some(repo) = crate::cache::repo_root_for(context.cwd.as_path()) else {
        return GitResolution::NotRepository;
    };

    if std::env::var("PANESHIP_DAEMON").is_ok() {
        return resolve_in_daemon(&repo);
    }

    if let Some(resolution) = crate::daemon::query_git(&repo, context.exit_code) {
        return resolution;
    }

    resolve_local(&repo)
}

fn resolve_in_daemon(repo: &Path) -> GitResolution {
    match crate::cache::git_lookup(repo) {
        GitLookup::Fresh(snapshot) => GitResolution::Ready {
            snapshot,
            stale: false,
        },
        GitLookup::Stale(Some(snapshot)) => GitResolution::Ready {
            snapshot,
            stale: true,
        },
        GitLookup::Stale(None) | GitLookup::Missing => GitResolution::Pending,
    }
}

fn resolve_local(repo: &Path) -> GitResolution {
    match crate::cache::git_lookup(repo) {
        GitLookup::Fresh(snapshot) => GitResolution::Ready {
            snapshot,
            stale: false,
        },
        GitLookup::Stale(Some(snapshot)) => GitResolution::Ready {
            snapshot,
            stale: true,
        },
        GitLookup::Stale(None) | GitLookup::Missing => {
            crate::cache::mark_git_pending(repo);
            match compute_git_status_with_timeout(repo, 30) {
                Some(snapshot) => {
                    crate::cache::set_git(repo, snapshot.clone());
                    GitResolution::Ready {
                        snapshot,
                        stale: false,
                    }
                }
                None => GitResolution::Pending,
            }
        }
    }
}

/// Compute Git status on a worker thread, giving up after `timeout_ms`.
pub fn compute_git_status_with_timeout(path: &Path, timeout_ms: u64) -> Option<GitSnapshot> {
    let path = path.to_path_buf();
    let (tx, rx) = std::sync::mpsc::channel();

    std::thread::spawn(move || {
        let _ = tx.send(compute_git_status_raw(&path));
    });

    rx.recv_timeout(Duration::from_millis(timeout_ms))
        .ok()
        .flatten()
}

/// Compute Git status directly with gix (blocking).
pub fn compute_git_status_raw(path: &Path) -> Option<GitSnapshot> {
    let repo = gix::discover(path).ok()?;
    let head = repo.head().ok()?;
    let head_id = head
        .id()
        .map(|id| id.to_string())
        .unwrap_or_else(|| "unknown".to_string());
    let branch = head
        .referent_name()
        .map(|name| name.shorten().to_string())
        .unwrap_or_else(|| {
            head.id()
                .map(|id| {
                    id.shorten()
                        .map(|s| s.to_string())
                        .unwrap_or_else(|_| "unknown".to_string())
                })
                .unwrap_or_else(|| "unknown".to_string())
        });

    let mut staged = 0;
    let mut unstaged = 0;
    let mut untracked = 0;

    if let Ok(status) = repo.status(gix::progress::Discard) {
        if let Ok(iter) = status.into_iter([]) {
            for item in iter.flatten() {
                use gix::status::Item;
                match item {
                    Item::TreeIndex(_change) => {
                        staged += 1;
                    }
                    Item::IndexWorktree(wt_item) => {
                        use gix::status::index_worktree::Item as WtItem;
                        match wt_item {
                            WtItem::DirectoryContents { .. } => {
                                untracked += 1;
                            }
                            _ => {
                                unstaged += 1;
                            }
                        }
                    }
                }
            }
        }
    }

    Some(GitSnapshot {
        branch,
        head_id,
        staged,
        unstaged,
        untracked,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::Config;
    use std::sync::Arc;

    fn context(git_state: Option<GitResolution>) -> PromptContext {
        PromptContext {
            cwd: std::env::temp_dir(),
            width: 80,
            exit_code: 0,
            last_command_duration_ms: None,
            config: Arc::new(Config::default()),
            git_state,
            language_state: None,
            package_state: None,
        }
    }

    #[test]
    fn pending_renders_explicit_placeholder() {
        let rendered = render_with_max_width(&context(Some(GitResolution::Pending)), 80);
        assert!(
            rendered.contains("git..."),
            "placeholder missing: {rendered:?}"
        );
        assert!(!rendered.is_empty());
    }

    #[test]
    fn not_repository_renders_nothing() {
        let rendered = render_with_max_width(&context(Some(GitResolution::NotRepository)), 80);
        assert!(rendered.is_empty());
    }

    #[test]
    fn ready_renders_branch_and_counts() {
        let snapshot = GitSnapshot {
            branch: "main".to_string(),
            head_id: "abc".to_string(),
            staged: 1,
            unstaged: 2,
            untracked: 3,
        };
        let resolution = GitResolution::Ready {
            snapshot,
            stale: false,
        };
        let rendered = render_with_max_width(&context(Some(resolution)), 80);
        assert!(rendered.contains("main"), "branch missing: {rendered:?}");
        assert!(rendered.contains('+'));
        assert!(rendered.contains('!'));
        assert!(rendered.contains('?'));
    }

    #[test]
    fn placeholder_is_dropped_when_too_narrow() {
        let rendered = render_with_max_width(&context(Some(GitResolution::Pending)), 3);
        assert!(rendered.is_empty());
    }
}
