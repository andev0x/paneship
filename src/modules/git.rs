use crate::cache::{get_or_compute_git, GitSnapshot};
use crate::core::layout::truncate_plain_to_width;
use crate::core::prompt::PromptContext;
use std::path::Path;
use unicode_width::UnicodeWidthStr;

pub fn render_with_max_width(context: &PromptContext, max_visible_width: usize) -> String {
    if max_visible_width == 0 {
        return String::new();
    }

    let snapshot = snapshot_for_context(context);
    let Some(snapshot) = snapshot else {
        return String::new();
    };

    let config = &context.config.git;

    let mut status_tokens = status_tokens(&snapshot, config);

    loop {
        let status_plain = status_tokens
            .iter()
            .map(|(plain, _)| plain.as_str())
            .collect::<Vec<_>>()
            .join(" ");

        let status_width = if status_plain.is_empty() {
            0
        } else {
            1 + UnicodeWidthStr::width(status_plain.as_str())
        };

        let icon_with_space = format!("{} ", config.branch_icon);
        let icon_width = UnicodeWidthStr::width(icon_with_space.as_str());
        let branch_budget = max_visible_width
            .saturating_sub(icon_width + status_width)
            .max(1);

        let branch = if UnicodeWidthStr::width(snapshot.branch.as_str()) > branch_budget {
            truncate_plain_to_width(snapshot.branch.as_str(), branch_budget)
        } else {
            snapshot.branch.clone()
        };

        let mut out = format!("\x1b[1;36m{} {}\x1b[0m", config.branch_icon, branch);
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
                "\x1b[1;36m{}\x1b[0m",
                truncate_plain_to_width(plain.as_str(), max_visible_width)
            );
        }

        status_tokens.pop();
    }
}

fn status_tokens(
    snapshot: &GitSnapshot,
    config: &crate::core::config::GitConfig,
) -> Vec<(String, String)> {
    let mut status = Vec::new();

    if snapshot.staged > 0 {
        let plain = format!("{}{}", config.staged_icon, snapshot.staged);
        let styled = format!("\x1b[32m{}\x1b[0m", plain);
        status.push((plain, styled));
    }
    if snapshot.unstaged > 0 {
        let plain = format!("{}{}", config.unstaged_icon, snapshot.unstaged);
        let styled = format!("\x1b[33m{}\x1b[0m", plain);
        status.push((plain, styled));
    }
    if snapshot.untracked > 0 {
        let plain = format!("{}{}", config.untracked_icon, snapshot.untracked);
        let styled = format!("\x1b[31m{}\x1b[0m", plain);
        status.push((plain, styled));
    }

    status
}

fn snapshot_for_context(context: &PromptContext) -> Option<GitSnapshot> {
    #[cfg(unix)]
    {
        crate::daemon::query_git(&context.cwd).or_else(|| {
            let fresh = get_or_compute_git(&context.cwd, || compute_git_status(&context.cwd));
            if let Some(ref s) = fresh {
                crate::daemon::notify_git(&context.cwd, s.clone());
            }
            fresh
        })
    }

    #[cfg(not(unix))]
    {
        get_or_compute_git(&context.cwd, || compute_git_status(&context.cwd))
    }
}

pub fn compute_git_status(path: &Path) -> Option<GitSnapshot> {
    let repo = gix::discover(path).ok()?;
    let head = repo.head().ok()?;
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
        staged,
        unstaged,
        untracked,
    })
}
