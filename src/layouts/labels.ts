// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab
// legends. physical keys never change; only what is printed on them does.

export type LabelFamily = 'qwerty' | 'azerty' | 'qwertz' | 'dvorak' | 'colemak';
export const FAMILIES: LabelFamily[] = ['qwerty', 'azerty', 'qwertz', 'dvorak', 'colemak'];

const QWERTY: Record<string, string> = {
  Backquote: '`', Minus: '-', Equal: '=', BracketLeft: '[', BracketRight: ']', Backslash: '\\',
  Semicolon: ';', Quote: "'", Comma: ',', Period: '.', Slash: '/', IntlBackslash: '\\',
};

const NAMED: Record<string, string> = {
  Escape: 'Esc', Backspace: 'Bksp', Tab: 'Tab', CapsLock: 'Caps', Enter: 'Enter', ShiftLeft: 'Shift', ShiftRight: 'Shift',
  ControlLeft: 'Ctrl', ControlRight: 'Ctrl', AltLeft: 'Alt', AltRight: 'Alt', MetaLeft: 'Win', MetaRight: 'Win',
  ContextMenu: 'Menu', Space: '', ArrowUp: '↑', ArrowDown: '↓', ArrowLeft: '←', ArrowRight: '→',
  PrintScreen: 'PrtSc', ScrollLock: 'ScrLk', Pause: 'Pause', Insert: 'Ins', Delete: 'Del', Home: 'Home', End: 'End',
  PageUp: 'PgUp', PageDown: 'PgDn', NumLock: 'Num', NumpadDivide: '/', NumpadMultiply: '*', NumpadSubtract: '-',
  NumpadAdd: '+', NumpadEnter: 'Ent', NumpadDecimal: '.',
};

/** overrides on top of qwerty, keyed by physical key */
const FAMILY: Record<Exclude<LabelFamily, 'qwerty'>, Record<string, string>> = {
  azerty: {
    Backquote: '²', Digit1: '&', Digit2: 'é', Digit3: '"', Digit4: "'", Digit5: '(', Digit6: '-', Digit7: 'è', Digit8: '_', Digit9: 'ç', Digit0: 'à',
    Minus: ')', Equal: '=', KeyQ: 'A', KeyW: 'Z', KeyA: 'Q', KeyZ: 'W', KeyM: ',', BracketLeft: '^', BracketRight: '$',
    Semicolon: 'M', Quote: 'ù', Backslash: '*', Comma: ';', Period: ':', Slash: '!', IntlBackslash: '<',
  },
  qwertz: {
    Backquote: '^', KeyY: 'Z', KeyZ: 'Y', Minus: 'ß', Equal: '´', BracketLeft: 'Ü', BracketRight: '+',
    Semicolon: 'Ö', Quote: 'Ä', Backslash: '#', Slash: '-', IntlBackslash: '<',
  },
  dvorak: {
    Minus: '[', Equal: ']', KeyQ: "'", KeyW: ',', KeyE: '.', KeyR: 'P', KeyT: 'Y', KeyY: 'F', KeyU: 'G', KeyI: 'C', KeyO: 'R', KeyP: 'L',
    BracketLeft: '/', BracketRight: '=', KeyS: 'O', KeyD: 'E', KeyF: 'U', KeyG: 'I', KeyH: 'D', KeyJ: 'H', KeyK: 'T', KeyL: 'N',
    Semicolon: 'S', Quote: '-', KeyZ: ';', KeyX: 'Q', KeyC: 'J', KeyV: 'K', KeyB: 'X', KeyN: 'B', KeyM: 'M', Comma: 'W', Period: 'V', Slash: 'Z',
  },
  colemak: {
    KeyE: 'F', KeyR: 'P', KeyT: 'G', KeyY: 'J', KeyU: 'L', KeyI: 'U', KeyO: 'Y', KeyP: ';',
    KeyS: 'R', KeyD: 'S', KeyF: 'T', KeyG: 'D', KeyJ: 'N', KeyK: 'E', KeyL: 'I', Semicolon: 'O', KeyN: 'K',
  },
};

export function staticLabel(code: string, family: LabelFamily): string {
  if (family !== 'qwerty' && code in FAMILY[family]) return FAMILY[family][code]!;
  if (code in NAMED) return NAMED[code]!;
  if (/^Key[A-Z]$/.test(code)) return code.slice(3);
  if (/^Digit\d$/.test(code)) return code.slice(5);
  if (/^Numpad\d$/.test(code)) return code.slice(6);
  if (/^F\d{1,2}$/.test(code)) return code;
  return QWERTY[code] ?? code;
}

/** `native` is the os's own map (code -> printed char), when we have one */
export function labelFor(code: string, family: LabelFamily, native: Map<string, string> | null): string {
  if (native && !(code in NAMED) && !/^(Numpad|F\d)/.test(code)) {
    const n = native.get(code);
    if (n) return n.length === 1 ? n.toUpperCase() : n;
  }
  return staticLabel(code, family);
}
