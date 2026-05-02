use std::{
    collections::{HashMap, HashSet},
    sync::OnceLock,
};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde::{Deserialize, Serialize};

pub const CONTEXT_DASHBOARD: &str = "dashboard";
pub const CONTEXT_USAGE_PAGE: &str = "usage_page";
pub const CONTEXT_CONFIG_PAGE: &str = "config_page";
pub const CONTEXT_SESSION_PAGE: &str = "session_page";
pub const CONTEXT_HELP: &str = "help";
pub const CONTEXT_CALL_DETAIL: &str = "call_detail";
pub const CONTEXT_PROJECT_PICKER: &str = "project_picker";
pub const CONTEXT_SESSION_PICKER: &str = "session_picker";
pub const CONTEXT_CURRENCY_PICKER: &str = "currency_picker";
pub const CONTEXT_DOWNLOAD_CONFIRM: &str = "download_confirm";
pub const CONTEXT_BUSY_MODAL: &str = "busy_modal";
pub const CONTEXT_EXPORT_PICKER: &str = "export_picker";
pub const CONTEXT_EXPORT_FOLDER_PICKER: &str = "export_folder_picker";
pub const CONTEXT_DESKTOP: &str = "desktop";
pub const CONTEXT_DESKTOP_SESSION_PAGE: &str = "desktop_session_page";
pub const CONTEXT_DESKTOP_MODAL: &str = "desktop_modal";
pub const CONTEXT_DESKTOP_CALL_DETAIL: &str = "desktop_call_detail";

pub const ACTION_QUIT: &str = "quit";
pub const ACTION_OPEN_HELP: &str = "open_help";
pub const ACTION_CLOSE_HELP: &str = "close_help";
pub const ACTION_CLOSE_CALL_DETAIL: &str = "close_call_detail";
pub const ACTION_CLOSE_MODAL: &str = "close_modal";
pub const ACTION_CANCEL: &str = "cancel";
pub const ACTION_CONFIRM: &str = "confirm";
pub const ACTION_NEXT_TAB: &str = "next_tab";
pub const ACTION_PREV_TAB: &str = "prev_tab";
pub const ACTION_PERIOD_TODAY: &str = "period_today";
pub const ACTION_PERIOD_WEEK: &str = "period_week";
pub const ACTION_PERIOD_THIRTY_DAYS: &str = "period_thirty_days";
pub const ACTION_PERIOD_MONTH: &str = "period_month";
pub const ACTION_PERIOD_ALL_TIME: &str = "period_all_time";
pub const ACTION_CYCLE_TOOL: &str = "cycle_tool";
pub const ACTION_CYCLE_SORT: &str = "cycle_sort";
pub const ACTION_TOGGLE_DATA_SOURCE: &str = "toggle_data_source";
pub const ACTION_OPEN_PROJECT_PICKER: &str = "open_project_picker";
pub const ACTION_OPEN_SESSION_PICKER: &str = "open_session_picker";
pub const ACTION_OPEN_EXPORT_PICKER: &str = "open_export_picker";
pub const ACTION_OPEN_EXPORT_FOLDER_PICKER: &str = "open_export_folder_picker";
pub const ACTION_PAGE_OVERVIEW: &str = "page_overview";
pub const ACTION_PAGE_DEEP_DIVE: &str = "page_deep_dive";
pub const ACTION_PAGE_USAGE: &str = "page_usage";
pub const ACTION_PAGE_CONFIG: &str = "page_config";
pub const ACTION_CLOSE_SESSION: &str = "close_session";
pub const ACTION_RELOAD: &str = "reload";
pub const ACTION_MOVE_UP: &str = "move_up";
pub const ACTION_MOVE_DOWN: &str = "move_down";
pub const ACTION_MOVE_PAGE_UP: &str = "move_page_up";
pub const ACTION_MOVE_PAGE_DOWN: &str = "move_page_down";
pub const ACTION_MOVE_HOME: &str = "move_home";
pub const ACTION_MOVE_END: &str = "move_end";
pub const ACTION_QUERY_BACKSPACE: &str = "query_backspace";
pub const ACTION_QUERY_CLEAR: &str = "query_clear";
pub const ACTION_GO_PARENT: &str = "go_parent";

const SUPPORTED_ACTIONS: &[&str] = &[
    ACTION_QUIT,
    ACTION_OPEN_HELP,
    ACTION_CLOSE_HELP,
    ACTION_CLOSE_CALL_DETAIL,
    ACTION_CLOSE_MODAL,
    ACTION_CANCEL,
    ACTION_CONFIRM,
    ACTION_NEXT_TAB,
    ACTION_PREV_TAB,
    ACTION_PERIOD_TODAY,
    ACTION_PERIOD_WEEK,
    ACTION_PERIOD_THIRTY_DAYS,
    ACTION_PERIOD_MONTH,
    ACTION_PERIOD_ALL_TIME,
    ACTION_CYCLE_TOOL,
    ACTION_CYCLE_SORT,
    ACTION_TOGGLE_DATA_SOURCE,
    ACTION_OPEN_PROJECT_PICKER,
    ACTION_OPEN_SESSION_PICKER,
    ACTION_OPEN_EXPORT_PICKER,
    ACTION_OPEN_EXPORT_FOLDER_PICKER,
    ACTION_PAGE_OVERVIEW,
    ACTION_PAGE_DEEP_DIVE,
    ACTION_PAGE_USAGE,
    ACTION_PAGE_CONFIG,
    ACTION_CLOSE_SESSION,
    ACTION_RELOAD,
    ACTION_MOVE_UP,
    ACTION_MOVE_DOWN,
    ACTION_MOVE_PAGE_UP,
    ACTION_MOVE_PAGE_DOWN,
    ACTION_MOVE_HOME,
    ACTION_MOVE_END,
    ACTION_QUERY_BACKSPACE,
    ACTION_QUERY_CLEAR,
    ACTION_GO_PARENT,
];

#[derive(Debug, Clone, Deserialize)]
pub struct Keymap {
    actions: Vec<ActionDef>,
    shortcuts: Vec<ShortcutDef>,
    help: Vec<HintGroup>,
    footers: HashMap<String, Vec<KeyHint>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ActionDef {
    pub id: String,
    pub label: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ShortcutDef {
    pub context: String,
    pub keys: Vec<String>,
    pub action: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HintGroup {
    pub title: String,
    pub items: Vec<KeyHint>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct KeyHint {
    pub keys: String,
    pub label: String,
    pub action: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct KeyInput {
    pub key: String,
    #[serde(default)]
    pub ctrl: bool,
    #[serde(default)]
    pub alt: bool,
    #[serde(default)]
    pub shift: bool,
    #[serde(default)]
    pub meta: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct NormalizedKey {
    code: String,
    ctrl: bool,
    alt: bool,
    shift: bool,
    meta: bool,
}

pub fn keymap() -> &'static Keymap {
    static KEYMAP: OnceLock<Keymap> = OnceLock::new();
    KEYMAP.get_or_init(|| {
        Keymap::from_json(include_str!("keymap.json"))
            .unwrap_or_else(|err| panic!("invalid embedded keymap: {err}"))
    })
}

impl Keymap {
    pub fn from_json(input: &str) -> Result<Self, String> {
        let keymap: Self =
            serde_json::from_str(input).map_err(|err| format!("parse keymap json: {err}"))?;
        keymap.validate()?;
        Ok(keymap)
    }

    pub fn actions(&self) -> &[ActionDef] {
        &self.actions
    }

    pub fn help_groups(&self) -> &[HintGroup] {
        &self.help
    }

    pub fn footer(&self, name: &str) -> &[KeyHint] {
        self.footers.get(name).map(Vec::as_slice).unwrap_or(&[])
    }

    pub fn resolve_tui(&self, context: &str, key: KeyEvent) -> Option<&str> {
        let input = NormalizedKey::from_tui(key)?;
        self.resolve_normalized(context, &input)
    }

    pub fn resolve_input(&self, context: &str, input: &KeyInput) -> Option<&str> {
        let input = NormalizedKey::from_input(input)?;
        self.resolve_normalized(context, &input)
    }

    fn resolve_normalized(&self, context: &str, input: &NormalizedKey) -> Option<&str> {
        self.shortcuts
            .iter()
            .filter(|shortcut| shortcut.context == context)
            .find(|shortcut| {
                shortcut.keys.iter().any(|key| {
                    parse_key(key)
                        .map(|candidate| candidate == *input)
                        .unwrap_or(false)
                })
            })
            .map(|shortcut| shortcut.action.as_str())
    }

    fn validate(&self) -> Result<(), String> {
        let supported: HashSet<&str> = SUPPORTED_ACTIONS.iter().copied().collect();
        let mut declared = HashSet::new();
        for action in &self.actions {
            if action.id.trim().is_empty() {
                return Err("action id cannot be empty".into());
            }
            if action.label.trim().is_empty() {
                return Err(format!("action {} has an empty label", action.id));
            }
            if !supported.contains(action.id.as_str()) {
                return Err(format!("unsupported action id {}", action.id));
            }
            if !declared.insert(action.id.as_str()) {
                return Err(format!("duplicate action id {}", action.id));
            }
        }

        if declared.is_empty() {
            return Err("keymap must declare at least one action".into());
        }

        let mut seen = HashMap::new();
        for shortcut in &self.shortcuts {
            if shortcut.context.trim().is_empty() {
                return Err(format!(
                    "shortcut for {} has an empty context",
                    shortcut.action
                ));
            }
            if shortcut.keys.is_empty() {
                return Err(format!(
                    "shortcut for {} in {} has no keys",
                    shortcut.action, shortcut.context
                ));
            }
            ensure_action(&declared, &shortcut.action)?;
            for key in &shortcut.keys {
                let parsed = parse_key(key)?;
                let signature = format!("{}:{}", shortcut.context, parsed.signature());
                if let Some(existing) = seen.insert(signature.clone(), shortcut.action.as_str()) {
                    return Err(format!(
                        "duplicate shortcut {signature} maps to both {existing} and {}",
                        shortcut.action
                    ));
                }
            }
        }

        for group in &self.help {
            if group.title.trim().is_empty() {
                return Err("help group title cannot be empty".into());
            }
            if group.items.is_empty() {
                return Err(format!("help group {} has no items", group.title));
            }
            for item in &group.items {
                ensure_hint("help", item, &declared)?;
            }
        }

        for (name, items) in &self.footers {
            if name.trim().is_empty() {
                return Err("footer name cannot be empty".into());
            }
            if items.is_empty() {
                return Err(format!("footer {name} has no items"));
            }
            for item in items {
                ensure_hint(name, item, &declared)?;
            }
        }

        Ok(())
    }
}

impl NormalizedKey {
    fn from_tui(key: KeyEvent) -> Option<Self> {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        let meta = key.modifiers.contains(KeyModifiers::SUPER);
        let mut shift = key.modifiers.contains(KeyModifiers::SHIFT);
        let code = match key.code {
            KeyCode::Backspace => "backspace".into(),
            KeyCode::Enter => "enter".into(),
            KeyCode::Left => "left".into(),
            KeyCode::Right => "right".into(),
            KeyCode::Up => "up".into(),
            KeyCode::Down => "down".into(),
            KeyCode::Home => "home".into(),
            KeyCode::End => "end".into(),
            KeyCode::PageUp => "pageup".into(),
            KeyCode::PageDown => "pagedown".into(),
            KeyCode::Tab => "tab".into(),
            KeyCode::BackTab => {
                shift = true;
                "tab".into()
            }
            KeyCode::Delete => "delete".into(),
            KeyCode::Esc => "esc".into(),
            KeyCode::Char(c) => {
                let code: String = c.to_lowercase().collect();
                shift = shifted_printable_supports_shift(&code) && (shift || c.is_uppercase());
                code
            }
            KeyCode::Null
            | KeyCode::Insert
            | KeyCode::F(_)
            | KeyCode::CapsLock
            | KeyCode::ScrollLock
            | KeyCode::NumLock
            | KeyCode::PrintScreen
            | KeyCode::Pause
            | KeyCode::Menu
            | KeyCode::KeypadBegin
            | KeyCode::Media(_)
            | KeyCode::Modifier(_) => return None,
        };

        Some(Self {
            code,
            ctrl,
            alt,
            shift,
            meta,
        })
    }

    fn from_input(input: &KeyInput) -> Option<Self> {
        let mut shift = input.shift;
        let code = canonical_code(&input.key)?;
        if is_printable_key(&code) && !shifted_printable_supports_shift(&code) {
            shift = false;
        }
        Some(Self {
            code,
            ctrl: input.ctrl,
            alt: input.alt,
            shift,
            meta: input.meta,
        })
    }

    fn signature(&self) -> String {
        let mut parts = Vec::new();
        if self.ctrl {
            parts.push("ctrl");
        }
        if self.alt {
            parts.push("alt");
        }
        if self.shift {
            parts.push("shift");
        }
        if self.meta {
            parts.push("meta");
        }
        parts.push(&self.code);
        parts.join("+")
    }
}

fn ensure_action<'a>(declared: &HashSet<&'a str>, action: &'a str) -> Result<(), String> {
    if declared.contains(action) {
        Ok(())
    } else {
        Err(format!("unknown action id {action}"))
    }
}

fn ensure_hint(context: &str, item: &KeyHint, declared: &HashSet<&str>) -> Result<(), String> {
    if item.keys.trim().is_empty() {
        return Err(format!("{context} hint for {} has empty keys", item.action));
    }
    if item.label.trim().is_empty() {
        return Err(format!(
            "{context} hint for {} has empty label",
            item.action
        ));
    }
    ensure_action(declared, &item.action)
}

fn parse_key(input: &str) -> Result<NormalizedKey, String> {
    let mut ctrl = false;
    let mut alt = false;
    let mut shift = false;
    let mut meta = false;
    let mut code = None;

    for part in input.split('+') {
        let part = part.trim();
        if part.is_empty() {
            return Err(format!("invalid empty key part in {input}"));
        }
        match part.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => ctrl = true,
            "alt" | "option" => alt = true,
            "shift" => shift = true,
            "meta" | "cmd" | "command" | "super" => meta = true,
            _ => {
                if code.replace(part.to_string()).is_some() {
                    return Err(format!("multiple key codes in {input}"));
                }
            }
        }
    }

    let code = code.ok_or_else(|| format!("missing key code in {input}"))?;
    let code = canonical_code(&code).ok_or_else(|| format!("unsupported key code {input}"))?;
    if is_printable_key(&code) && !shifted_printable_supports_shift(&code) {
        shift = false;
    }

    Ok(NormalizedKey {
        code,
        ctrl,
        alt,
        shift,
        meta,
    })
}

fn canonical_code(input: &str) -> Option<String> {
    let trimmed = input.trim();
    let lower = trimmed.to_ascii_lowercase();
    let code = match lower.as_str() {
        "esc" | "escape" => "esc",
        "enter" | "return" => "enter",
        "tab" => "tab",
        "backspace" => "backspace",
        "left" | "arrowleft" => "left",
        "right" | "arrowright" => "right",
        "up" | "arrowup" => "up",
        "down" | "arrowdown" => "down",
        "home" => "home",
        "end" => "end",
        "pgup" | "pageup" => "pageup",
        "pgdn" | "pagedown" => "pagedown",
        "space" | "spacebar" => " ",
        _ if trimmed.chars().count() == 1 => return Some(trimmed.to_lowercase()),
        _ => return None,
    };
    Some(code.into())
}

fn is_printable_key(code: &str) -> bool {
    code.chars().count() == 1
}

fn shifted_printable_supports_shift(code: &str) -> bool {
    is_printable_key(code) && code.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_MINIMAL: &str = r#"
        {
          "actions": [{ "id": "quit", "label": "quit" }],
          "shortcuts": [{ "context": "dashboard", "keys": ["q"], "action": "quit" }],
          "help": [{ "title": "General", "items": [{ "keys": "q", "label": "quit", "action": "quit" }] }],
          "footers": { "dashboard": [{ "keys": "q", "label": "quit", "action": "quit" }] }
        }
    "#;

    #[test]
    fn embedded_keymap_loads_and_resolves_tui_keys() {
        let keymap = keymap();

        let action = keymap.resolve_tui(
            CONTEXT_DASHBOARD,
            KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE),
        );

        assert_eq!(action, Some(ACTION_CYCLE_SORT));
        assert!(!keymap.help_groups().is_empty());
        assert!(!keymap.footer("desktop").is_empty());
    }

    #[test]
    fn resolves_desktop_shift_tab() {
        let keymap = keymap();
        let action = keymap.resolve_input(
            CONTEXT_DASHBOARD,
            &KeyInput {
                key: "Tab".into(),
                ctrl: false,
                alt: false,
                shift: true,
                meta: false,
            },
        );

        assert_eq!(action, Some(ACTION_PREV_TAB));
    }

    #[test]
    fn resolves_shifted_printable_shortcut_without_stealing_plain_key() {
        let keymap = keymap();

        let shifted = keymap.resolve_input(
            CONTEXT_DASHBOARD,
            &KeyInput {
                key: "D".into(),
                ctrl: false,
                alt: false,
                shift: true,
                meta: false,
            },
        );
        let plain = keymap.resolve_input(
            CONTEXT_DASHBOARD,
            &KeyInput {
                key: "d".into(),
                ctrl: false,
                alt: false,
                shift: false,
                meta: false,
            },
        );

        assert_eq!(shifted, Some(ACTION_TOGGLE_DATA_SOURCE));
        assert_eq!(plain, Some(ACTION_PAGE_DEEP_DIVE));
    }

    #[test]
    fn rejects_duplicate_context_keys() {
        let json = r#"
            {
              "actions": [{ "id": "quit", "label": "quit" }],
              "shortcuts": [
                { "context": "dashboard", "keys": ["q"], "action": "quit" },
                { "context": "dashboard", "keys": ["Q"], "action": "quit" }
              ],
              "help": [{ "title": "General", "items": [{ "keys": "q", "label": "quit", "action": "quit" }] }],
              "footers": { "dashboard": [{ "keys": "q", "label": "quit", "action": "quit" }] }
            }
        "#;

        let err = Keymap::from_json(json).unwrap_err();

        assert!(err.contains("duplicate shortcut"));
    }

    #[test]
    fn rejects_unsupported_action_ids() {
        let json = r#"
            {
              "actions": [{ "id": "not_real", "label": "not real" }],
              "shortcuts": [],
              "help": [],
              "footers": {}
            }
        "#;

        let err = Keymap::from_json(json).unwrap_err();

        assert!(err.contains("unsupported action id not_real"));
    }

    #[test]
    fn rejects_unknown_shortcut_actions() {
        let json = r#"
            {
              "actions": [{ "id": "quit", "label": "quit" }],
              "shortcuts": [{ "context": "dashboard", "keys": ["q"], "action": "open_help" }],
              "help": [{ "title": "General", "items": [{ "keys": "q", "label": "quit", "action": "quit" }] }],
              "footers": { "dashboard": [{ "keys": "q", "label": "quit", "action": "quit" }] }
            }
        "#;

        let err = Keymap::from_json(json).unwrap_err();

        assert!(err.contains("unknown action id open_help"));
    }

    #[test]
    fn rejects_unknown_help_and_footer_actions() {
        let help_json = r#"
            {
              "actions": [{ "id": "quit", "label": "quit" }],
              "shortcuts": [{ "context": "dashboard", "keys": ["q"], "action": "quit" }],
              "help": [{ "title": "General", "items": [{ "keys": "h", "label": "help", "action": "open_help" }] }],
              "footers": { "dashboard": [{ "keys": "q", "label": "quit", "action": "quit" }] }
            }
        "#;
        let footer_json = r#"
            {
              "actions": [{ "id": "quit", "label": "quit" }],
              "shortcuts": [{ "context": "dashboard", "keys": ["q"], "action": "quit" }],
              "help": [{ "title": "General", "items": [{ "keys": "q", "label": "quit", "action": "quit" }] }],
              "footers": { "dashboard": [{ "keys": "h", "label": "help", "action": "open_help" }] }
            }
        "#;

        assert!(Keymap::from_json(help_json)
            .unwrap_err()
            .contains("unknown action id open_help"));
        assert!(Keymap::from_json(footer_json)
            .unwrap_err()
            .contains("unknown action id open_help"));
    }

    #[test]
    fn minimal_valid_keymap_loads() {
        let keymap = Keymap::from_json(VALID_MINIMAL).unwrap();

        assert_eq!(keymap.actions()[0].id, ACTION_QUIT);
    }
}
