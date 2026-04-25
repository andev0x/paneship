use crate::core::prompt::PromptContext;
use crate::modules::directory::Directory;
use crate::modules::git::Git;
use crate::modules::status::Status;
use crate::modules::Module;

pub fn render(context: &PromptContext) -> String {
    let modules: Vec<Box<dyn Module>> = vec![Box::new(Directory), Box::new(Git), Box::new(Status)];

    let mut output = String::new();
    for (i, module) in modules.iter().enumerate() {
        let rendered = module.render(context);
        if !rendered.is_empty() {
            if i > 0 && !output.is_empty() && !rendered.starts_with(' ') && !output.ends_with(' ') {
                output.push(' ');
            }
            output.push_str(&rendered);
        }
    }

    // Add a trailing space for the cursor
    output.push(' ');
    output
}
