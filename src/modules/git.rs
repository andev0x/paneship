use crate::cache::{get_or_compute_git, GitSnapshot};
use crate::core::prompt::PromptContext;
use crate::modules::Module;
use std::path::Path;

pub struct Git;

impl Module for Git {
    fn render(&self, context: &PromptContext) -> String {
        let snapshot = crate::daemon::query_git(&context.cwd).or_else(|| {
            let fresh = get_or_compute_git(&context.cwd, || compute_git_status(&context.cwd));
            if let Some(ref s) = fresh {
                crate::daemon::notify_git(&context.cwd, s.clone());
            }
            fresh
        });

        let config = &context.config.git;

        if let Some(snapshot) = snapshot {
            let mut status = Vec::new();
            if snapshot.staged > 0 {
                status.push(format!(
                    "\x1b[32m{}{}\x1b[0m",
                    config.staged_icon, snapshot.staged
                ));
            }
            if snapshot.unstaged > 0 {
                status.push(format!(
                    "\x1b[33m{}{}\x1b[0m",
                    config.unstaged_icon, snapshot.unstaged
                ));
            }
            if snapshot.untracked > 0 {
                status.push(format!(
                    "\x1b[31m{}{}\x1b[0m",
                    config.untracked_icon, snapshot.untracked
                ));
            }

            let status_str = if status.is_empty() {
                "".to_string()
            } else {
                format!(" [{}]", status.join(" "))
            };

            format!(
                " \x1b[35mon\x1b[0m \x1b[1;36m{} {}\x1b[0m{}",
                config.branch_icon, snapshot.branch, status_str
            )
        } else {
            "".to_string()
        }
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
