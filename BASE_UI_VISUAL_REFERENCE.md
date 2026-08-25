# Incular Controls visual reference

Incular Controls ships a polished, neutral preset inspired by the examples
published in the Base UI documentation. Base UI itself is unstyled; this file
records Incular's translation rather than claiming a Base UI theme.

Reference: [base-ui.com](https://base-ui.com/) and the v1.7 component examples
(accessed 2026-08-24). The examples consistently use compact logical-pixel
controls, a one-pixel neutral border, restrained radii, high-contrast text,
subtle hover/pressed surface changes, and a visible accent focus ring.

## Translation into Incular

| Example language | Incular token |
| --- | --- |
| neutral canvas/surface | `ControlTheme::palette` (`colors` compatibility alias) |
| compact controls | `ControlTheme::button` / `input` and `ControlDensity` |
| small radii | `ControlTheme::radius` and component radius tokens |
| focus outline | `ControlColors::focus_ring` + `StateColor::focused` |
| popup elevation | `ControlTheme::elevation.popup` / `PopupTheme` |
| selected/checked accent | `ControlColors::selection` / `accent` |

No CSS units, browser outlines, box-sizing rules, or icon fonts are imported.
Geometry remains logical pixels and icons use retained vector paths from
`incular_widgets::icons`.

