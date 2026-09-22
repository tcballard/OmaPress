# Workspace polish development handoff

Base: `e0aea4fd42891c52268399f62f17fdf6e39f1027` (main, retrieved 2026-09-22).

## Changes

Explicit Material toolbar edge padding and height keep controls clear of compositor-rounded corners. Search and new-article controls share a 36px height. Formatting controls use the same button component as the workspace actions, with visible keyboard focus. Editor title, summary, banner and body share responsive horizontal insets. Banner instructions reserve space for actions. Placeholder colours are opaque theme-derived colours; accent buttons choose black or white text by luminance.

Qt renders an opaque window. On Lua-based Hyprland, the app asynchronously registers a session-only opacity rule restricted to the exact `omapress`/`OmaPress` classes, including inactive and fullscreen states. It never edits user configuration. Registration is guarded against duplicates and restored on activation following a compositor config reload. IPC has a one-second deadline; non-Hyprland sessions skip it. `OMAPRESS_KEEP_COMPOSITOR_OPACITY=1` skips registration (reload Hyprland first if a previous launch already registered the session rule). This targets current Quattro/Lua Hyprland; legacy Hyprland is not covered by this integration.

## Reproduced now

- Loaded the actual changed QML with PySide6 / Qt 6.11.2, software offscreen rendering, using a fixture backend. Captured and visually inspected Familiar light and dark at 900 and 1440px. Header: 72px, left/right inset 24px, vertical inset 18px. Search and add controls: 36px, 8px gap.
- `git diff --check`: passed.
- Native regression coverage extends the existing theme test to an opened editor, control geometry, window alpha and a fake `hyprctl` executable verifying the scoped request and opt-out. Run through the normal native CI job.

## Not run locally

This editing runtime has no Qt C++ development headers/CMake. Native compilation and regression tests are delegated to repository CI; consult this PR's checks for results. Offscreen fixture captures do not establish live Wayland acceptance or publishing behaviour.

## Live acceptance

On the XPS, check Familiar in tiled and floating windows, focused and unfocused, with terminal text underneath. Confirm no bleed-through, comfortable rounded-corner insets and no overlap at minimum width. Switch to a dark theme, reload Hyprland and refocus the app. Confirm keyboard focus, typing, Markdown/Preview, add/replace/remove banner and undo/redo still work.

No document format or storage changes. Existing install, build, rollback and removal instructions remain in `getting-started.md` and `development.md`. This is a development PR, not a released version.

Upstream contract checked: Hyprland wiki `content/configuring/core/rules/window-rules.md`, blob `980460f709b8cac91497c85e6471366b9bc57c45`, and `using-hyprctl.md`, blob `cdae466ff816dfdd71dbcf8e7cc89777038be66f`.
