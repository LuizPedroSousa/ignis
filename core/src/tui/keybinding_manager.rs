use super::input::InputAction;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::collections::HashMap;
use std::time::Instant;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KeyPress {
    pub code: KeyCode,
    pub modifiers: KeyModifiers,
}

impl KeyPress {
    pub fn new(code: KeyCode, modifiers: KeyModifiers) -> Self {
        Self { code, modifiers }
    }

    pub fn from_key_event(key: KeyEvent) -> Self {
        Self {
            code: key.code,
            modifiers: key.modifiers,
        }
    }

    pub fn from_char(c: char) -> Self {
        Self {
            code: KeyCode::Char(c),
            modifiers: KeyModifiers::empty(),
        }
    }

    pub fn from_string(s: &str) -> Option<Self> {
        match s {
            "Space" => Some(Self::new(KeyCode::Char(' '), KeyModifiers::empty())),
            "Backslash" => Some(Self::new(KeyCode::Char('\\'), KeyModifiers::empty())),
            "Enter" => Some(Self::new(KeyCode::Enter, KeyModifiers::empty())),
            "Esc" => Some(Self::new(KeyCode::Esc, KeyModifiers::empty())),
            s if s.len() == 1 => {
                let c = s.chars().next()?;
                Some(Self::from_char(c))
            }
            _ => None,
        }
    }

    pub fn to_display_string(&self) -> String {
        match &self.code {
            KeyCode::Char(' ') => "<Space>".to_string(),
            KeyCode::Char(c) => c.to_string(),
            KeyCode::Enter => "<Enter>".to_string(),
            KeyCode::Esc => "<Esc>".to_string(),
            KeyCode::Backspace => "<Backspace>".to_string(),
            _ => format!("{:?}", self.code),
        }
    }
}

pub type KeySequence = Vec<KeyPress>;

#[derive(Debug, Clone, Copy)]
pub enum SequenceMatch {
    Complete(InputAction),
    Partial,
    NoMatch,
}

pub struct KeyBindingManager {
    leader_key: KeyPress,
    sequence_timeout: u64,
    leader_bindings: HashMap<KeyPress, InputAction>,
    vim_sequences: HashMap<KeySequence, InputAction>,
    single_key_bindings: HashMap<KeyPress, InputAction>,
    enable_leader: bool,
}

impl KeyBindingManager {
    pub fn new(
        leader_key: KeyPress,
        sequence_timeout: u64,
        enable_leader: bool,
    ) -> Self {
        let mut manager = Self {
            leader_key,
            sequence_timeout,
            leader_bindings: HashMap::new(),
            vim_sequences: HashMap::new(),
            single_key_bindings: HashMap::new(),
            enable_leader,
        };

        manager.setup_default_bindings();
        manager
    }

    fn setup_default_bindings(&mut self) {
        if self.enable_leader {
            self.leader_bindings.insert(
                KeyPress::from_char('f'),
                InputAction::OpenFile,
            );
            self.leader_bindings.insert(
                KeyPress::from_char('w'),
                InputAction::WriteLogs,
            );
            self.leader_bindings.insert(
                KeyPress::from_char('c'),
                InputAction::CleanBuild,
            );
            self.leader_bindings.insert(
                KeyPress::from_char('r'),
                InputAction::Rebuild,
            );
            self.leader_bindings.insert(
                KeyPress::from_char('q'),
                InputAction::Quit,
            );
            self.leader_bindings.insert(
                KeyPress::from_char('b'),
                InputAction::OpenBuildMenu,
            );
            self.leader_bindings.insert(
                KeyPress::from_char('e'),
                InputAction::OpenExecMenu,
            );
            self.leader_bindings.insert(
                KeyPress::from_char('?'),
                InputAction::ShowHelp,
            );
        }

        self.vim_sequences.insert(
            vec![KeyPress::from_char('g'), KeyPress::from_char('f')],
            InputAction::OpenFile,
        );
        self.vim_sequences.insert(
            vec![KeyPress::from_char('y'), KeyPress::from_char('y')],
            InputAction::YankLine,
        );
        self.vim_sequences.insert(
            vec![KeyPress::from_char('g'), KeyPress::from_char('g')],
            InputAction::ScrollToTop,
        );
        self.vim_sequences.insert(
            vec![KeyPress::from_char('z'), KeyPress::from_char('z')],
            InputAction::ScrollToMiddle,
        );
        self.vim_sequences.insert(
            vec![KeyPress::from_char('z'), KeyPress::from_char('t')],
            InputAction::ScrollToViewportTop,
        );
        self.vim_sequences.insert(
            vec![KeyPress::from_char('z'), KeyPress::from_char('b')],
            InputAction::ScrollToViewportBottom,
        );

        self.single_key_bindings.insert(
            KeyPress::from_char('q'),
            InputAction::Quit,
        );
        self.single_key_bindings.insert(
            KeyPress::from_char('b'),
            InputAction::OpenBuildMenu,
        );
        self.single_key_bindings.insert(
            KeyPress::from_char('e'),
            InputAction::OpenExecMenu,
        );
        self.single_key_bindings.insert(
            KeyPress::new(KeyCode::Char('1'), KeyModifiers::ALT),
            InputAction::SwitchTab(0),
        );
        self.single_key_bindings.insert(
            KeyPress::new(KeyCode::Char('2'), KeyModifiers::ALT),
            InputAction::SwitchTab(1),
        );
        self.single_key_bindings.insert(
            KeyPress::new(KeyCode::Char('3'), KeyModifiers::ALT),
            InputAction::SwitchTab(2),
        );
        self.single_key_bindings.insert(
            KeyPress::new(KeyCode::Char('4'), KeyModifiers::ALT),
            InputAction::SwitchTab(3),
        );
        self.single_key_bindings.insert(
            KeyPress::new(KeyCode::Char('5'), KeyModifiers::ALT),
            InputAction::SwitchTab(4),
        );
        self.single_key_bindings.insert(
            KeyPress::from_char(':'),
            InputAction::EnterCommand,
        );
        self.single_key_bindings.insert(
            KeyPress::from_char('/'),
            InputAction::EnterSearch,
        );
        self.single_key_bindings.insert(
            KeyPress::from_char('n'),
            InputAction::NextSearch,
        );
        self.single_key_bindings.insert(
            KeyPress::new(KeyCode::Char('N'), KeyModifiers::SHIFT),
            InputAction::PrevSearch,
        );
        self.single_key_bindings.insert(
            KeyPress::new(KeyCode::Char('G'), KeyModifiers::SHIFT),
            InputAction::ScrollToBottom,
        );
        self.single_key_bindings.insert(
            KeyPress::from_char('j'),
            InputAction::ScrollDown,
        );
        self.single_key_bindings.insert(
            KeyPress::new(KeyCode::Down, KeyModifiers::empty()),
            InputAction::ScrollDown,
        );
        self.single_key_bindings.insert(
            KeyPress::from_char('k'),
            InputAction::ScrollUp,
        );
        self.single_key_bindings.insert(
            KeyPress::new(KeyCode::Up, KeyModifiers::empty()),
            InputAction::ScrollUp,
        );
        self.single_key_bindings.insert(
            KeyPress::new(KeyCode::Char('d'), KeyModifiers::CONTROL),
            InputAction::ScrollHalfPageDown,
        );
        self.single_key_bindings.insert(
            KeyPress::new(KeyCode::Char('u'), KeyModifiers::CONTROL),
            InputAction::ScrollHalfPageUp,
        );
        self.single_key_bindings.insert(
            KeyPress::new(KeyCode::Char('f'), KeyModifiers::CONTROL),
            InputAction::ScrollPageDown,
        );
        self.single_key_bindings.insert(
            KeyPress::new(KeyCode::Char('b'), KeyModifiers::CONTROL),
            InputAction::ScrollPageUp,
        );
        self.single_key_bindings.insert(
            KeyPress::new(KeyCode::PageDown, KeyModifiers::empty()),
            InputAction::ScrollPageDown,
        );
        self.single_key_bindings.insert(
            KeyPress::new(KeyCode::PageUp, KeyModifiers::empty()),
            InputAction::ScrollPageUp,
        );
        self.single_key_bindings.insert(
            KeyPress::new(KeyCode::Home, KeyModifiers::empty()),
            InputAction::ScrollToTop,
        );
        self.single_key_bindings.insert(
            KeyPress::new(KeyCode::End, KeyModifiers::empty()),
            InputAction::ScrollToBottom,
        );
        self.single_key_bindings.insert(
            KeyPress::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
            InputAction::Quit,
        );
        self.single_key_bindings.insert(
            KeyPress::new(KeyCode::Char('L'), KeyModifiers::SHIFT),
            InputAction::NextTab,
        );
        self.single_key_bindings.insert(
            KeyPress::new(KeyCode::Char('H'), KeyModifiers::SHIFT),
            InputAction::PrevTab,
        );
    }

    pub fn is_leader_key(&self, key: &KeyPress) -> bool {
        self.enable_leader && key == &self.leader_key
    }

    pub fn match_sequence(&self, sequence: &KeySequence) -> SequenceMatch {
        if sequence.is_empty() {
            return SequenceMatch::NoMatch;
        }

        if sequence.len() == 1 && self.is_leader_key(&sequence[0]) {
            return SequenceMatch::Partial;
        }

        if sequence.len() == 2 && self.is_leader_key(&sequence[0]) {
            if let Some(action) = self.leader_bindings.get(&sequence[1]) {
                return SequenceMatch::Complete(*action);
            }
            return SequenceMatch::NoMatch;
        }

        if let Some(action) = self.vim_sequences.get(sequence) {
            return SequenceMatch::Complete(*action);
        }

        for (vim_seq, _) in &self.vim_sequences {
            if vim_seq.len() > sequence.len()
                && vim_seq[..sequence.len()] == sequence[..] {
                return SequenceMatch::Partial;
            }
        }

        SequenceMatch::NoMatch
    }

    pub fn match_single_key(&self, key: &KeyPress) -> Option<InputAction> {
        self.single_key_bindings.get(key).copied()
    }

    pub fn get_sequence_timeout(&self) -> u64 {
        self.sequence_timeout
    }

    pub fn add_leader_binding(&mut self, key: KeyPress, action: InputAction) {
        self.leader_bindings.insert(key, action);
    }

    pub fn add_vim_sequence(&mut self, sequence: KeySequence, action: InputAction) {
        self.vim_sequences.insert(sequence, action);
    }

    pub fn add_single_key_binding(&mut self, key: KeyPress, action: InputAction) {
        self.single_key_bindings.insert(key, action);
    }
}

impl Default for KeyBindingManager {
    fn default() -> Self {
        Self::new(
            KeyPress::from_char(' '),
            1000,
            true,
        )
    }
}

#[derive(Debug)]
pub struct PendingSequence {
    pub keys: KeySequence,
    pub timestamp: Instant,
}

impl PendingSequence {
    pub fn new(key: KeyPress) -> Self {
        Self {
            keys: vec![key],
            timestamp: Instant::now(),
        }
    }

    pub fn add_key(&mut self, key: KeyPress) {
        self.keys.push(key);
        self.timestamp = Instant::now();
    }

    pub fn is_timeout(&self, timeout_ms: u64) -> bool {
        self.timestamp.elapsed().as_millis() > timeout_ms as u128
    }

    pub fn get_display_string(&self) -> String {
        self.keys
            .iter()
            .map(|k| k.to_display_string())
            .collect::<Vec<_>>()
            .join("")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_leader_sequence_matching() {
        let manager = KeyBindingManager::default();

        let leader = KeyPress::from_char(' ');
        let f_key = KeyPress::from_char('f');

        let sequence = vec![leader.clone()];
        match manager.match_sequence(&sequence) {
            SequenceMatch::Partial => {},
            _ => panic!("Expected partial match for leader key"),
        }

        let sequence = vec![leader, f_key];
        match manager.match_sequence(&sequence) {
            SequenceMatch::Complete(InputAction::OpenFile) => {},
            _ => panic!("Expected complete match for <Space>f"),
        }
    }

    #[test]
    fn test_vim_sequence_matching() {
        let manager = KeyBindingManager::default();

        let g_key = KeyPress::from_char('g');
        let f_key = KeyPress::from_char('f');

        let sequence = vec![g_key.clone()];
        match manager.match_sequence(&sequence) {
            SequenceMatch::Partial => {},
            _ => panic!("Expected partial match for 'g'"),
        }

        let sequence = vec![g_key, f_key];
        match manager.match_sequence(&sequence) {
            SequenceMatch::Complete(InputAction::OpenFile) => {},
            _ => panic!("Expected complete match for 'gf'"),
        }
    }

    #[test]
    fn test_single_key_matching() {
        let manager = KeyBindingManager::default();

        let q_key = KeyPress::from_char('q');
        match manager.match_single_key(&q_key) {
            Some(InputAction::Quit) => {},
            _ => panic!("Expected quit action for 'q'"),
        }
    }

    #[test]
    fn test_key_display() {
        let space = KeyPress::from_char(' ');
        assert_eq!(space.to_display_string(), "<Space>");

        let f = KeyPress::from_char('f');
        assert_eq!(f.to_display_string(), "f");
    }

    #[test]
    fn test_pending_sequence_display() {
        let mut seq = PendingSequence::new(KeyPress::from_char(' '));
        assert_eq!(seq.get_display_string(), "<Space>");

        seq.add_key(KeyPress::from_char('f'));
        assert_eq!(seq.get_display_string(), "<Space>f");
    }
}
