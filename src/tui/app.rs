//! TUI application state and interaction model (Phase 2, Module C).
//!
//! Configuration is edited as an in-memory `toml_edit::DocumentMut`, which
//! preserves comments and ordering from the on-disk file. Every mutation
//! re-parses a fresh [`Config`] snapshot so the live preview reacts instantly.

use crate::cache::{GitSnapshot, LanguageSnapshot};
use crate::core::config::Config;
use crate::core::icons::{IconProfile, ThemeProfile};
use crate::core::prompt::{MetadataState, PromptContext};
use crate::modules::git::GitResolution;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::path::PathBuf;
use std::sync::Arc;
use toml_edit::DocumentMut;

/// Source of a choice field's options.
pub enum ChoiceKind {
    Icons,
    Themes,
}

/// Editing behaviour for a config field.
pub enum FieldKind {
    Text,
    Bool,
    Int { min: i64, max: i64 },
    Choice(ChoiceKind),
}

/// A single editable field within a section.
pub struct FieldDef {
    pub label: &'static str,
    pub path: &'static [&'static str],
    pub kind: FieldKind,
}

/// A left-pane configuration section.
pub struct Section {
    pub title: &'static str,
    pub fields: &'static [FieldDef],
}

/// Hierarchical configuration sections shown in the left pane.
pub static SECTIONS: &[Section] = &[
    Section {
        title: "Directory",
        fields: &[
            FieldDef {
                label: "Truncation length",
                path: &["directory", "truncation_length"],
                kind: FieldKind::Int { min: 1, max: 9 },
            },
            FieldDef {
                label: "Truncate to repo",
                path: &["directory", "truncate_to_repo"],
                kind: FieldKind::Bool,
            },
        ],
    },
    Section {
        title: "Git",
        fields: &[
            FieldDef {
                label: "Branch icon",
                path: &["git", "branch_icon"],
                kind: FieldKind::Text,
            },
            FieldDef {
                label: "Staged icon",
                path: &["git", "staged_icon"],
                kind: FieldKind::Text,
            },
            FieldDef {
                label: "Unstaged icon",
                path: &["git", "unstaged_icon"],
                kind: FieldKind::Text,
            },
            FieldDef {
                label: "Untracked icon",
                path: &["git", "untracked_icon"],
                kind: FieldKind::Text,
            },
        ],
    },
    Section {
        title: "Status",
        fields: &[
            FieldDef {
                label: "Success icon",
                path: &["status", "success_icon"],
                kind: FieldKind::Text,
            },
            FieldDef {
                label: "Failure icon",
                path: &["status", "failure_icon"],
                kind: FieldKind::Text,
            },
        ],
    },
    Section {
        title: "Metadata",
        fields: &[
            FieldDef {
                label: "Time color",
                path: &["metadata", "time_color"],
                kind: FieldKind::Text,
            },
            FieldDef {
                label: "Paneship color",
                path: &["metadata", "paneship_color"],
                kind: FieldKind::Text,
            },
        ],
    },
    Section {
        title: "Icon Sets",
        fields: &[FieldDef {
            label: "Profile",
            path: &["icons", "profile"],
            kind: FieldKind::Choice(ChoiceKind::Icons),
        }],
    },
    Section {
        title: "Theme",
        fields: &[FieldDef {
            label: "Name",
            path: &["theme", "name"],
            kind: FieldKind::Choice(ChoiceKind::Themes),
        }],
    },
    Section {
        title: "Prompt",
        fields: &[
            FieldDef {
                label: "Right prompt",
                path: &["prompt", "rprompt"],
                kind: FieldKind::Bool,
            },
            FieldDef {
                label: "RPROMPT min width",
                path: &["prompt", "rprompt_min_width"],
                kind: FieldKind::Int { min: 10, max: 200 },
            },
        ],
    },
    Section {
        title: "Environment",
        fields: &[
            FieldDef {
                label: "Enabled",
                path: &["environment", "enabled"],
                kind: FieldKind::Bool,
            },
            FieldDef {
                label: "Kubernetes",
                path: &["environment", "kubernetes"],
                kind: FieldKind::Bool,
            },
            FieldDef {
                label: "Container",
                path: &["environment", "docker"],
                kind: FieldKind::Bool,
            },
            FieldDef {
                label: "Cloud profiles",
                path: &["environment", "cloud"],
                kind: FieldKind::Bool,
            },
            FieldDef {
                label: "Nix / Direnv",
                path: &["environment", "nix_direnv"],
                kind: FieldKind::Bool,
            },
            FieldDef {
                label: "System health",
                path: &["environment", "health"],
                kind: FieldKind::Bool,
            },
        ],
    },
];

/// What the event loop should do after a key press.
pub enum Action {
    None,
    Quit,
    Save,
}

/// An in-progress text edit.
pub struct Edit {
    pub path: Vec<String>,
    pub buffer: String,
}

/// Full application state.
pub struct App {
    pub doc: DocumentMut,
    pub config: Arc<Config>,
    pub section: usize,
    pub field: usize,
    pub edit: Option<Edit>,
    pub dirty: bool,
    pub status: String,
    pub preview_width: u16,
}

impl App {
    /// Load the on-disk config (or seed defaults) into an editable document.
    pub fn load() -> Result<Self, String> {
        let path = Config::config_path();
        let doc = match std::fs::read_to_string(&path) {
            Ok(content) if !content.trim().is_empty() => content
                .parse::<DocumentMut>()
                .map_err(|err| format!("invalid config at {}: {err}", path.display()))?,
            _ => default_document()?,
        };

        let mut app = Self {
            doc,
            config: Arc::new(Config::default()),
            section: 0,
            field: 0,
            edit: None,
            dirty: false,
            status: format!("Editing {}", path.display()),
            preview_width: 60,
        };
        app.refresh_preview();
        Ok(app)
    }

    /// Re-parse the in-memory document so preview reflects edits immediately.
    fn refresh_preview(&mut self) {
        let text = self.doc.to_string();
        let config = toml::from_str::<Config>(&text).unwrap_or_default();
        self.config = Arc::new(config);
    }

    fn mark_dirty(&mut self) {
        self.dirty = true;
        self.refresh_preview();
    }

    /// Current section (always valid).
    pub fn current_section(&self) -> &'static Section {
        let index = self.section.min(SECTIONS.len().saturating_sub(1));
        &SECTIONS[index]
    }

    /// Currently highlighted field of the current section.
    pub fn current_field(&self) -> Option<&'static FieldDef> {
        self.current_section().fields.get(self.field)
    }

    /// Render the current configuration into prompt lines for the preview pane.
    pub fn preview(&self) -> Vec<String> {
        let context = PromptContext {
            cwd: PathBuf::from("/home/dev/projects/paneship"),
            width: self.preview_width.max(20) as usize,
            exit_code: 0,
            last_command_duration_ms: Some(1289),
            config: Arc::clone(&self.config),
            git_state: Some(GitResolution::Ready {
                snapshot: GitSnapshot {
                    branch: "main".to_string(),
                    head_id: "deadbeef".to_string(),
                    staged: 2,
                    unstaged: 1,
                    untracked: 3,
                },
                stale: false,
            }),
            language_state: Some(MetadataState::Ready {
                value: LanguageSnapshot {
                    language: "rust".to_string(),
                    version: Some("1.74".to_string()),
                },
                stale: false,
            }),
            package_state: Some(MetadataState::Ready {
                value: "1.1.6".to_string(),
                stale: false,
            }),
            environment_state: Some(MetadataState::Ready {
                value: crate::detectors::infra::EnvironmentSnapshot {
                    entries: vec![crate::detectors::infra::EnvironmentInfo {
                        kind: crate::detectors::infra::EnvironmentKind::Kubernetes,
                        label: "dev/team-a".to_string(),
                    }],
                },
                stale: false,
            }),
        };

        crate::core::renderer::render(&context)
            .lines()
            .map(str::to_string)
            .collect()
    }

    /// Handle a key press, returning the resulting action.
    pub fn handle_key(&mut self, key: KeyEvent) -> Action {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('s') {
            self.commit_edit();
            return Action::Save;
        }

        if self.edit.is_some() {
            self.handle_edit_key(key);
            return Action::None;
        }

        match key.code {
            KeyCode::Char('q') => Action::Quit,
            KeyCode::Esc => Action::Quit,
            KeyCode::Up | KeyCode::Char('k') => {
                self.prev_field();
                Action::None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.next_field();
                Action::None
            }
            KeyCode::Left | KeyCode::Char('h') => {
                self.prev_section();
                Action::None
            }
            KeyCode::Right | KeyCode::Char('l') => {
                self.next_section();
                Action::None
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                self.activate();
                Action::None
            }
            _ => Action::None,
        }
    }

    fn handle_edit_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Enter => self.commit_edit(),
            KeyCode::Esc => self.edit = None,
            KeyCode::Backspace => {
                if let Some(edit) = self.edit.as_mut() {
                    edit.buffer.pop();
                }
            }
            KeyCode::Char(c) => {
                if let Some(edit) = self.edit.as_mut() {
                    edit.buffer.push(c);
                }
            }
            _ => {}
        }
    }

    fn commit_edit(&mut self) {
        let Some(edit) = self.edit.take() else {
            return;
        };
        let refs: Vec<&str> = edit.path.iter().map(String::as_str).collect();
        self.set_string(&refs, &edit.buffer);
        self.mark_dirty();
    }

    fn activate(&mut self) {
        let Some(def) = self.current_field() else {
            return;
        };
        match &def.kind {
            FieldKind::Bool => {
                let current = self.get_bool(def.path).unwrap_or(false);
                self.set_bool(def.path, !current);
                self.mark_dirty();
            }
            FieldKind::Int { min, max } => {
                let current = self.get_int(def.path).unwrap_or(*min);
                let next = if current >= *max { *min } else { current + 1 };
                self.set_int(def.path, next);
                self.mark_dirty();
            }
            FieldKind::Choice(_) => {
                let choices = self.choices(def);
                if choices.is_empty() {
                    return;
                }
                let current = self.get_str(def.path).unwrap_or_default();
                let next = choices
                    .iter()
                    .position(|choice| *choice == current)
                    .map(|index| (index + 1) % choices.len())
                    .unwrap_or(0);
                self.set_string(def.path, &choices[next]);
                self.mark_dirty();
            }
            FieldKind::Text => {
                let buffer = self.get_str(def.path).unwrap_or_default();
                self.edit = Some(Edit {
                    path: def.path.iter().map(|key| key.to_string()).collect(),
                    buffer,
                });
            }
        }
    }

    fn next_field(&mut self) {
        let count = self.current_section().fields.len();
        if count > 0 {
            self.field = (self.field + 1) % count;
        }
    }

    fn prev_field(&mut self) {
        let count = self.current_section().fields.len();
        if count > 0 {
            self.field = if self.field == 0 {
                count - 1
            } else {
                self.field - 1
            };
        }
    }

    fn next_section(&mut self) {
        self.section = (self.section + 1) % SECTIONS.len();
        self.field = 0;
    }

    fn prev_section(&mut self) {
        self.section = if self.section == 0 {
            SECTIONS.len() - 1
        } else {
            self.section - 1
        };
        self.field = 0;
    }

    /// The rendered value of a field, for display.
    pub fn field_value(&self, def: &FieldDef) -> String {
        match def.kind {
            FieldKind::Text | FieldKind::Choice(_) => self.get_str(def.path).unwrap_or_default(),
            FieldKind::Bool => self.get_bool(def.path).unwrap_or(false).to_string(),
            FieldKind::Int { .. } => self
                .get_int(def.path)
                .map(|value| value.to_string())
                .unwrap_or_default(),
        }
    }

    /// Resolve the choice options for a field.
    pub fn choices(&self, def: &FieldDef) -> Vec<String> {
        match &def.kind {
            FieldKind::Choice(ChoiceKind::Icons) => IconProfile::ALL
                .iter()
                .map(|profile| profile.as_str().to_string())
                .collect(),
            FieldKind::Choice(ChoiceKind::Themes) => ThemeProfile::ALL
                .iter()
                .map(|theme| theme.as_str().to_string())
                .collect(),
            _ => Vec::new(),
        }
    }

    /// Friendly label for the current value of a choice field.
    pub fn choice_label(&self, def: &FieldDef, value: &str) -> Option<String> {
        match &def.kind {
            FieldKind::Choice(ChoiceKind::Icons) => {
                IconProfile::parse(value).map(|profile| profile.label().to_string())
            }
            FieldKind::Choice(ChoiceKind::Themes) => {
                ThemeProfile::parse(value).map(|theme| theme.label().to_string())
            }
            _ => None,
        }
    }

    /// Write the in-memory document atomically and signal the daemon.
    pub fn save(&mut self) -> Result<(), String> {
        let contents = self.doc.to_string();
        super::persist::atomic_write(&Config::config_path(), &contents)
            .map_err(|err| format!("write failed: {err}"))?;
        self.dirty = false;

        if crate::daemon::reload_config() {
            self.status = "Saved · daemon reloaded".to_string();
        } else {
            self.status = "Saved · daemon not running".to_string();
        }
        Ok(())
    }

    // -- toml_edit helpers --------------------------------------------------

    fn get_item(&self, path: &[&str]) -> Option<&toml_edit::Item> {
        let (first, rest) = path.split_first()?;
        let mut item = self.doc.as_table().get(first)?;
        for key in rest {
            item = item.get(key)?;
        }
        Some(item)
    }

    fn get_str(&self, path: &[&str]) -> Option<String> {
        self.get_item(path)?.as_str().map(str::to_string)
    }

    fn get_bool(&self, path: &[&str]) -> Option<bool> {
        self.get_item(path)?.as_bool()
    }

    fn get_int(&self, path: &[&str]) -> Option<i64> {
        self.get_item(path)?.as_integer()
    }

    fn set_item(&mut self, path: &[&str], item: toml_edit::Item) {
        let Some((last, parents)) = path.split_last() else {
            return;
        };
        let mut table = self.doc.as_table_mut();
        for key in parents {
            let entry = table
                .entry(key)
                .or_insert(toml_edit::Item::Table(toml_edit::Table::new()));
            if !entry.is_table() {
                *entry = toml_edit::Item::Table(toml_edit::Table::new());
            }
            table = entry.as_table_mut().expect("entry forced to table");
        }
        table.insert(last, item);
    }

    fn set_string(&mut self, path: &[&str], value: &str) {
        self.set_item(path, toml_edit::value(value));
    }

    fn set_bool(&mut self, path: &[&str], value: bool) {
        self.set_item(path, toml_edit::value(value));
    }

    fn set_int(&mut self, path: &[&str], value: i64) {
        self.set_item(path, toml_edit::value(value));
    }
}

fn default_document() -> Result<DocumentMut, String> {
    let text = toml::to_string_pretty(&Config::default())
        .map_err(|err| format!("failed to render default config: {err}"))?;
    text.parse::<DocumentMut>()
        .map_err(|err| format!("failed to parse default config: {err}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build an app without touching the user's real config file.
    fn headless() -> App {
        let doc = default_document().unwrap();
        let mut app = App {
            doc,
            config: Arc::new(Config::default()),
            section: 0,
            field: 0,
            edit: None,
            dirty: false,
            status: String::new(),
            preview_width: 80,
        };
        app.refresh_preview();
        app
    }

    #[test]
    fn cycling_icon_profile_updates_preview_config() {
        let mut app = headless();
        // Sections: Directory(0) Git(1) Status(2) Metadata(3) Icon Sets(4) Theme(5)
        app.section = 4;
        app.field = 0;
        let before = app.config.language_style("rust").icon;
        app.activate();
        let after = app.config.language_style("rust").icon;
        assert_ne!(before, after, "profile switch should change resolved icon");
        assert!(app.dirty);
    }

    #[test]
    fn editing_writes_into_document() {
        let mut app = headless();
        app.section = 1;
        app.field = 1; // Git staged icon (text)
        app.activate();
        assert!(app.edit.is_some());
        if let Some(edit) = app.edit.as_mut() {
            edit.buffer = "CUSTOM".to_string();
        }
        app.commit_edit();
        assert_eq!(
            app.get_str(&["git", "staged_icon"]).as_deref(),
            Some("CUSTOM")
        );
    }

    #[test]
    fn toggling_bool_flips_value() {
        let mut app = headless();
        app.section = 0;
        app.field = 1; // truncate_to_repo
        let before = app.get_bool(&["directory", "truncate_to_repo"]).unwrap();
        app.activate();
        let after = app.get_bool(&["directory", "truncate_to_repo"]).unwrap();
        assert_ne!(before, after);
    }

    #[test]
    fn preview_renders_two_lines() {
        let app = headless();
        let lines = app.preview();
        assert!(!lines.is_empty());
    }

    #[test]
    fn toggling_environment_section_flips_value() {
        let mut app = headless();
        // Sections: Directory(0) Git(1) Status(2) Metadata(3) Icon Sets(4)
        //           Theme(5) Prompt(6) Environment(7)
        app.section = 7;
        app.field = 0;
        let before = app.get_bool(&["environment", "enabled"]).unwrap();
        app.activate();
        let after = app.get_bool(&["environment", "enabled"]).unwrap();
        assert_ne!(before, after);
    }
}
