use crossterm::event::{KeyCode, KeyEvent};
use super::keybinding_manager::{KeyBindingManager, KeyPress, SequenceMatch};
use super::vim::VimCommandMode;

#[derive(Debug, Clone, Copy)]
pub enum InputAction {
    Quit,
    SwitchTab(usize),
    NextTab,
    PrevTab,
    EnterCommand,
    EnterSearch,
    ExecuteCommand,
    ExecuteSearch,
    CancelInput,
    InsertChar(char),
    Backspace,
    NextSearch,
    PrevSearch,
    OpenFile,
    YankLine,
    OpenBuildMenu,
    OpenExecMenu,
    ScrollUp,
    ScrollDown,
    ScrollPageUp,
    ScrollPageDown,
    ScrollHalfPageUp,
    ScrollHalfPageDown,
    ScrollToTop,
    ScrollToBottom,
    ScrollToMiddle,
    ScrollToViewportTop,
    ScrollToViewportMiddle,
    ScrollToViewportBottom,
    ScrollUpCount(usize),
    ScrollDownCount(usize),
    ScrollPageUpCount(usize),
    ScrollPageDownCount(usize),
    ScrollHalfPageUpCount(usize),
    ScrollHalfPageDownCount(usize),
    WriteLogs,
    CleanBuild,
    Rebuild,
    ShowHelp,
    RestartExec,
    KillExec,
    None,
}

pub fn handle_key_event(
    key: KeyEvent,
    vim_mode: &mut VimCommandMode,
    keybindings: &KeyBindingManager,
    is_command_mode: bool,
    is_search_mode: bool,
) -> InputAction {
    if is_command_mode || is_search_mode {
        return handle_input_mode(key, is_command_mode);
    }

    let key_press = KeyPress::from_key_event(key);

    if vim_mode.is_sequence_timeout(keybindings.get_sequence_timeout()) {
        vim_mode.clear_sequence();
        vim_mode.clear_count();
    }

    if let KeyCode::Char(c) = key.code {
        if c.is_ascii_digit() && !vim_mode.pending_sequence.is_some() && key.modifiers.is_empty() {
            if vim_mode.has_count() || c != '0' {
                vim_mode.push_count_digit(c);
                return InputAction::None;
            }
        }
    }

    if let Some(pending) = &vim_mode.pending_sequence {
        let mut sequence = pending.keys.clone();
        sequence.push(key_press.clone());

        match keybindings.match_sequence(&sequence) {
            SequenceMatch::Complete(action) => {
                vim_mode.clear_sequence();
                let count = vim_mode.get_count();
                vim_mode.clear_count();
                return apply_count_to_action(action, count);
            }
            SequenceMatch::Partial => {
                vim_mode.add_to_sequence(key_press);
                return InputAction::None;
            }
            SequenceMatch::NoMatch => {
                vim_mode.clear_sequence();
                vim_mode.clear_count();
                return InputAction::None;
            }
        }
    }

    if keybindings.is_leader_key(&key_press) {
        vim_mode.start_sequence(key_press);
        return InputAction::None;
    }

    let sequence = vec![key_press.clone()];
    match keybindings.match_sequence(&sequence) {
        SequenceMatch::Complete(action) => {
            let count = vim_mode.get_count();
            vim_mode.clear_count();
            return apply_count_to_action(action, count);
        }
        SequenceMatch::Partial => {
            vim_mode.start_sequence(key_press);
            return InputAction::None;
        }
        SequenceMatch::NoMatch => {}
    }

    if let Some(action) = keybindings.match_single_key(&key_press) {
        let count = vim_mode.get_count();
        vim_mode.clear_count();
        return apply_count_to_action(action, count);
    }

    vim_mode.clear_count();
    InputAction::None
}

fn handle_input_mode(key: KeyEvent, is_command: bool) -> InputAction {
    match key.code {
        KeyCode::Enter => {
            if is_command {
                InputAction::ExecuteCommand
            } else {
                InputAction::ExecuteSearch
            }
        }
        KeyCode::Esc => InputAction::CancelInput,
        KeyCode::Backspace => InputAction::Backspace,
        KeyCode::Char(c) => InputAction::InsertChar(c),
        _ => InputAction::None,
    }
}

fn apply_count_to_action(action: InputAction, count: usize) -> InputAction {
    match action {
        InputAction::ScrollUp => InputAction::ScrollUpCount(count),
        InputAction::ScrollDown => InputAction::ScrollDownCount(count),
        InputAction::ScrollPageUp => InputAction::ScrollPageUpCount(count),
        InputAction::ScrollPageDown => InputAction::ScrollPageDownCount(count),
        InputAction::ScrollHalfPageUp => InputAction::ScrollHalfPageUpCount(count),
        InputAction::ScrollHalfPageDown => InputAction::ScrollHalfPageDownCount(count),
        _ => action,
    }
}
