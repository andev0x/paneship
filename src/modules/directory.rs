use crate::core::prompt::PromptContext;
use crate::modules::Module;

pub struct Directory;

impl Module for Directory {
    fn render(&self, context: &PromptContext) -> String {
        let home = std::env::var("HOME").map(std::path::PathBuf::from).ok();
        let path = &context.cwd;

        let mut display_path = if let Some(home_path) = home {
            if let Ok(rel) = path.strip_prefix(&home_path) {
                format!("~/{}", rel.display())
            } else {
                path.display().to_string()
            }
        } else {
            path.display().to_string()
        };

        // Remove trailing slash if not root
        if display_path.len() > 1 && display_path.ends_with('/') {
            display_path.pop();
        }

        // Basic truncation: if path is longer than 30% of width, truncate it
        let max_dir_len = (context.width as f64 * 0.4) as usize;
        if display_path.len() > max_dir_len && max_dir_len > 10 {
            let parts: Vec<&str> = display_path.split('/').collect();
            if parts.len() > 3 {
                display_path = format!("{}/.../{}", parts[0], parts[parts.len() - 1]);
            }
        }

        format!("\x1b[1;34m\u{f07b} {}\x1b[0m", display_path)
    }
}
