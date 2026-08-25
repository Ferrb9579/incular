# Controls migration notes

The original convenience API remains valid. Recommended incremental changes:

* Wrap feature roots in `ControlThemeScope` instead of passing themes through
  every constructor.
* Prefer `Input` as the new name for single-line `TextField`; the alias is
  intentionally source-compatible.
* Use `checkbox::Root`, `switch::Root`, and `radio::Root` when replacing a
  monolithic selection control with custom anatomy.
* Replace glyph check marks and chevrons with `incular_widgets::Icon` and the
  retained `incular_widgets::icons` paths.
* Use `Separator` for semantic horizontal/vertical separators; keep `Divider`
  when the legacy margin-bearing visual is desired.
* Use `StateColor`/`StateTable` instead of duplicating hover and pressed logic
  in each component.

