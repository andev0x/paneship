use crate::cache::{get_or_compute_git, GitSnapshot};
use crate::core::prompt::PromptContext;
use crate::modules::Module;
use std::path::Path;

pub struct Git;

impl Module for Git {
    fn render(&self, context: &PromptContext) -> String {
        let snapshot = get_or_compute_git(&context.cwd, || compute_git_status(&context.cwd));

        if let Some(snapshot) = snapshot {
            let branch = &snapshot.branch;
            let dirty = if snapshot.is_dirty() { "*" } else { "" };
            format!(" \x1b[35mon\x1b[0m \x1b[1;36m {}\x1b[0m{}", branch, dirty)
        } else {
            "".to_string()
        }
    }
}

fn compute_git_status(path: &Path) -> Option<GitSnapshot> {
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

    // Simplest dirty check for now
    let is_dirty = repo
        .status(gix::progress::Discard)
        .ok()
        .and_then(|s| s.into_index_worktree_iter(None).ok())
        .map(|mut iter| iter.next().is_some())
        .unwrap_or(false);
    
    Some(GitSnapshot {
        branch,
        staged: if is_dirty { 1 } else { 0 },
        unstaged: 0,
        untracked: 0,
    })
}
