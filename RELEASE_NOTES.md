inputlab 0.1.0, the first release. a native desktop input tester: it finds every keyboard, mouse, controller, midi device and hid gadget, shows each one live, and measures polling rate and dpi from real raw input. no accounts, no network, everything stays on your machine.

## download

| system | file | status |
| --- | --- | --- |
| windows | the `.msi` or the `-setup.exe` | primary |
| macos | the `.dmg` (apple silicon or intel) | experimental: lists devices, no live keyboard or mouse input yet |
| linux | the `.AppImage` or the `.deb` | experimental: needs read access to `/dev/input` (the `input` group) |

the builds are unsigned. windows smartscreen may warn the first time: more info, then run anyway.

## before you test anything

start it once with `--no-capture`. nothing is locked, hooked or swallowed, and the exit button works in every test. ctrl+alt+del always works.

a normal test captures the device under test, and you leave it differently depending on the device:

- **mouse**: hold esc for about 1 s. there is no exit button on screen at all.
- **keyboard**: click the exit button, or hold ctrl+alt+shift+backspace for about 1.5 s (from any connected keyboard).
- **controller, hid, midi**: click the exit button. controller buttons never exit.

if the window loses focus, the ui stops answering, or the app crashes, everything is released.

## what is in it

- keyboards: full, tkl, 75%, 65% and 60%, ansi and iso, tested-keys checklist, rollover, chatter detector, analog depth (wooting profile, experimental)
- mice: polling rate from raw timestamps (checked from 125 hz to 8 khz in tests), dpi test, sensor jitter and angle snapping, button and wheel tests, a model that follows your mouse
- controllers: dualsense, dualshock 4, xbox, switch pro and generic layouts, stick circularity, deadzone, drift, polling, rumble, battery
- generic hid with a live hex dump, and midi
- session summary with json export and copy-as-text

## known limits, said plainly

- the windows lockdown (cursor clip, hooks, esc-hold, exit combo) compiles and its logic is unit tested, but it has not yet been run on a real machine by the author. use `--no-capture` first and report anything odd.
- not checked on real hardware: 4k and 8k mice, analog keyboards (the wooting report format is unverified), specific controllers, rumble and battery.
- ctrl+alt+del, win+l and other os-reserved combos cannot be captured. the keyboard view lists them as reserved.
- mouse and controller silhouettes are grip families, not product renders.

## licence

source viewable, not reusable. you may read it, learn from it and build your own app, but not copy the project or pieces of its code. see LICENSE.
