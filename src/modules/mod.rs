use crate::core::prompt::PromptContext;

pub mod directory;
pub mod git;
pub mod status;

pub trait Module {
    fn render(&self, context: &PromptContext) -> String;
}
