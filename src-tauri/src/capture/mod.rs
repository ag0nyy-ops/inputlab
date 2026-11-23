//! test-session capture rules. this module is pure logic (no os calls) so the
//! exit rules, the part that must never trap the user, are unit tested.
//!
//!   mouse under test    -> hold esc ~1s. nothing the mouse does can leave.
//!   keyboard under test -> exit button, or hold ctrl+alt+shift+backspace ~1.5s
//!   anything else       -> exit button only. gamepad buttons never exit.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::keys::{is_alt, is_ctrl, is_shift};
use crate::platform::{CaptureMode, SrcId};

pub const ESC_HOLD_DEFAULT_MS: u32 = 1000;
pub const COMBO_HOLD_DEFAULT_MS: u32 = 1500;

#[derive(Deserialize, Clone, Copy, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct SessionSettings {
    pub esc_hold_ms: u32,
    pub combo_hold_ms: u32,
}

impl Default for SessionSettings {
    fn default() -> Self {
        Self { esc_hold_ms: ESC_HOLD_DEFAULT_MS, combo_hold_ms: COMBO_HOLD_DEFAULT_MS }
    }
}

impl SessionSettings {
    /// the frontend is untrusted input: clamp so a bad value can neither make
    /// the exit instant by accident nor so slow it is unusable.
    pub fn sanitized(self) -> Self {
        Self {
            esc_hold_ms: self.esc_hold_ms.clamp(300, 5000),
            combo_hold_ms: self.combo_hold_ms.clamp(500, 6000),
        }
    }
}

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ExitKind {
    Esc,
    Combo,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExitRule {
    EscHold { hold_ns: u64 },
    Combo { hold_ns: u64 },
    /// exit button only
    None,
}

impl ExitRule {
    pub fn for_mode(mode: CaptureMode, s: &SessionSettings) -> Self {
        match mode {
            CaptureMode::Mouse => ExitRule::EscHold { hold_ns: s.esc_hold_ms as u64 * 1_000_000 },
            CaptureMode::Keyboard => ExitRule::Combo { hold_ns: s.combo_hold_ms as u64 * 1_000_000 },
            CaptureMode::Other => ExitRule::None,
        }
    }

    pub fn kind(&self) -> Option<ExitKind> {
        match self {
            ExitRule::EscHold { .. } => Some(ExitKind::Esc),
            ExitRule::Combo { .. } => Some(ExitKind::Combo),
            ExitRule::None => None,
        }
    }

    /// does the on-screen exit button end the session in this mode?
    pub fn button_allowed(mode: CaptureMode) -> bool {
        mode != CaptureMode::Mouse
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ExitState {
    /// 0.0 .. 1.0
    pub progress: f32,
    pub fired: bool,
}

/// watches key state from every keyboard (not just the one under test) and
/// turns "held long enough" into an exit.
pub struct ExitTracker {
    rule: ExitRule,
    down: HashMap<SrcId, HashSet<&'static str>>,
    since: Option<u64>,
    fired: bool,
}

impl ExitTracker {
    pub fn new(rule: ExitRule) -> Self {
        Self { rule, down: HashMap::new(), since: None, fired: false }
    }

    pub fn rule(&self) -> ExitRule {
        self.rule
    }

    pub fn key(&mut self, src: SrcId, code: &'static str, down: bool, now: u64) {
        let set = self.down.entry(src).or_default();
        if down {
            set.insert(code);
        } else {
            set.remove(code);
        }
        self.refresh(now);
    }

    /// a keyboard went away mid-hold: forget what it was holding.
    pub fn forget_source(&mut self, src: SrcId, now: u64) {
        self.down.remove(&src);
        self.refresh(now);
    }

    fn any(&self, f: impl Fn(&str) -> bool) -> bool {
        self.down.values().flatten().any(|k| f(k))
    }

    fn condition(&self) -> bool {
        match self.rule {
            ExitRule::None => false,
            ExitRule::EscHold { .. } => self.any(|k| k == "Escape"),
            ExitRule::Combo { .. } => {
                self.any(is_ctrl) && self.any(is_alt) && self.any(is_shift) && self.any(|k| k == "Backspace")
            }
        }
    }

    fn refresh(&mut self, now: u64) {
        if self.fired {
            return;
        }
        if self.condition() {
            self.since.get_or_insert(now);
        } else {
            self.since = None;
        }
    }

    pub fn tick(&mut self, now: u64) -> ExitState {
        let hold = match self.rule {
            ExitRule::EscHold { hold_ns } | ExitRule::Combo { hold_ns } => hold_ns,
            ExitRule::None => return ExitState { progress: 0.0, fired: false },
        };
        if self.fired {
            return ExitState { progress: 1.0, fired: true };
        }
        let Some(since) = self.since else {
            return ExitState { progress: 0.0, fired: false };
        };
        let held = now.saturating_sub(since);
        if held >= hold {
            self.fired = true;
            return ExitState { progress: 1.0, fired: true };
        }
        ExitState { progress: held as f32 / hold as f32, fired: false }
    }
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ExitInfo {
    /// which keyboard rule ends the session, if any
    pub kind: Option<ExitKind>,
    pub hold_ms: u32,
    /// the on-screen exit button exists and works
    pub button: bool,
    /// keys of the combo, for the persistent hint
    pub combo: Vec<&'static str>,
}

pub fn exit_info(mode: CaptureMode, s: &SessionSettings, capture_disabled: bool) -> ExitInfo {
    let rule = ExitRule::for_mode(mode, s);
    ExitInfo {
        kind: rule.kind(),
        hold_ms: match rule {
            ExitRule::EscHold { .. } => s.esc_hold_ms,
            ExitRule::Combo { .. } => s.combo_hold_ms,
            ExitRule::None => 0,
        },
        // with --no-capture the mouse is not captured, so a button is safe and useful
        button: ExitRule::button_allowed(mode) || capture_disabled,
        combo: if mode == CaptureMode::Keyboard {
            vec!["ControlLeft", "AltLeft", "ShiftLeft", "Backspace"]
        } else {
            vec![]
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MS: u64 = 1_000_000;

    fn esc() -> ExitTracker {
        ExitTracker::new(ExitRule::EscHold { hold_ns: 1000 * MS })
    }
    fn combo() -> ExitTracker {
        ExitTracker::new(ExitRule::Combo { hold_ns: 1500 * MS })
    }

    #[test]
    fn esc_hold_fires_after_hold_time() {
        let mut t = esc();
        t.key(1, "Escape", true, 0);
        assert!(!t.tick(500 * MS).fired);
        let s = t.tick(500 * MS);
        assert!((s.progress - 0.5).abs() < 0.01);
        assert!(!t.tick(999 * MS).fired);
        assert!(t.tick(1000 * MS).fired);
    }

    #[test]
    fn esc_release_resets_progress() {
        let mut t = esc();
        t.key(1, "Escape", true, 0);
        assert!(t.tick(900 * MS).progress > 0.8);
        t.key(1, "Escape", false, 900 * MS);
        assert_eq!(t.tick(950 * MS).progress, 0.0);
        // pressing again starts a fresh hold, not the old one
        t.key(1, "Escape", true, 1000 * MS);
        assert!(!t.tick(1900 * MS).fired);
        assert!(t.tick(2000 * MS).fired);
    }

    #[test]
    fn auto_repeat_does_not_restart_the_hold() {
        let mut t = esc();
        t.key(1, "Escape", true, 0);
        for i in 1..10 {
            t.key(1, "Escape", true, i * 100 * MS); // os key repeat
        }
        assert!(t.tick(1000 * MS).fired);
    }

    #[test]
    fn mouse_session_ignores_other_keys() {
        let mut t = esc();
        for k in ["KeyA", "Enter", "ControlLeft", "Backspace"] {
            t.key(1, k, true, 0);
        }
        assert!(!t.tick(10_000 * MS).fired);
    }

    #[test]
    fn combo_needs_all_four_held() {
        let mut t = combo();
        t.key(1, "ControlLeft", true, 0);
        t.key(1, "AltLeft", true, 0);
        t.key(1, "ShiftLeft", true, 0);
        assert!(!t.tick(5000 * MS).fired, "three of four must not exit");
        t.key(1, "Backspace", true, 5000 * MS);
        assert!(!t.tick(6000 * MS).fired);
        assert!(t.tick(6500 * MS).fired);
    }

    #[test]
    fn combo_resets_when_any_key_is_released() {
        for released in ["ControlLeft", "AltLeft", "ShiftLeft", "Backspace"] {
            let mut t = combo();
            for k in ["ControlLeft", "AltLeft", "ShiftLeft", "Backspace"] {
                t.key(1, k, true, 0);
            }
            assert!(t.tick(1400 * MS).progress > 0.9);
            t.key(1, released, false, 1400 * MS);
            assert_eq!(t.tick(1450 * MS).progress, 0.0, "releasing {released}");
            assert!(!t.tick(9000 * MS).fired);
        }
    }

    #[test]
    fn combo_accepts_right_hand_modifiers() {
        let mut t = combo();
        for k in ["ControlRight", "AltRight", "ShiftRight", "Backspace"] {
            t.key(1, k, true, 0);
        }
        assert!(t.tick(1500 * MS).fired);
    }

    #[test]
    fn combo_works_from_any_keyboard() {
        // the keyboard under test (src 1) is dead; the combo comes from keyboard 2
        let mut t = combo();
        for k in ["ControlLeft", "AltLeft", "ShiftLeft", "Backspace"] {
            t.key(2, k, true, 0);
        }
        assert!(t.tick(1500 * MS).fired);
    }

    #[test]
    fn combo_may_be_split_across_keyboards() {
        let mut t = combo();
        t.key(1, "ControlLeft", true, 0);
        t.key(2, "AltLeft", true, 0);
        t.key(2, "ShiftLeft", true, 0);
        t.key(1, "Backspace", true, 0);
        assert!(t.tick(1500 * MS).fired);
    }

    #[test]
    fn unplugging_a_keyboard_drops_its_held_keys() {
        let mut t = combo();
        for k in ["ControlLeft", "AltLeft", "ShiftLeft", "Backspace"] {
            t.key(1, k, true, 0);
        }
        t.forget_source(1, 500 * MS);
        assert!(!t.tick(9000 * MS).fired);
    }

    #[test]
    fn exit_button_only_modes_never_fire_from_keys() {
        let mut t = ExitTracker::new(ExitRule::None);
        for k in ["Escape", "ControlLeft", "AltLeft", "ShiftLeft", "Backspace"] {
            t.key(1, k, true, 0);
        }
        let s = t.tick(60_000 * MS);
        assert!(!s.fired);
        assert_eq!(s.progress, 0.0);
    }

    #[test]
    fn fired_state_is_latched() {
        let mut t = esc();
        t.key(1, "Escape", true, 0);
        assert!(t.tick(1000 * MS).fired);
        t.key(1, "Escape", false, 1100 * MS);
        assert!(t.tick(1200 * MS).fired);
    }

    #[test]
    fn rules_per_mode() {
        let s = SessionSettings::default();
        assert_eq!(ExitRule::for_mode(CaptureMode::Mouse, &s).kind(), Some(ExitKind::Esc));
        assert_eq!(ExitRule::for_mode(CaptureMode::Keyboard, &s).kind(), Some(ExitKind::Combo));
        assert_eq!(ExitRule::for_mode(CaptureMode::Other, &s).kind(), None);
    }

    #[test]
    fn mouse_mode_has_no_exit_button_but_others_do() {
        let s = SessionSettings::default();
        assert!(!exit_info(CaptureMode::Mouse, &s, false).button);
        assert!(exit_info(CaptureMode::Keyboard, &s, false).button);
        assert!(exit_info(CaptureMode::Other, &s, false).button);
        // --no-capture frees the mouse, so the button is allowed
        assert!(exit_info(CaptureMode::Mouse, &s, true).button);
    }

    #[test]
    fn settings_are_clamped() {
        let s = SessionSettings { esc_hold_ms: 0, combo_hold_ms: 10_000_000 }.sanitized();
        assert_eq!(s.esc_hold_ms, 300);
        assert_eq!(s.combo_hold_ms, 6000);
    }
}
