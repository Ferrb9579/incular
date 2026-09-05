# incular-config

## Architecture and support

| Contract | Status |
| --- | --- |
| Ownership | Shared constraints, alignment, insets, localization and application policies. |
| API class | application; re-exports and path overrides follow the [architecture contract](../../docs/ARCHITECTURE.md). |
| Support | Available; writable invariant fields migrate in G. |

Renderer-independent configuration values shared across Incular layers.

This crate owns box constraints, physical edge insets, axis and direction
values, and alignment/flex/wrap policy enums. It depends only on
`incular-core` geometry, so layout, widgets, runtime, and platform adapters can
exchange these values without depending on one another. It deliberately owns
no layout algorithms, widget state, renderer state, or platform handles.
`RuntimeEnvironment` keeps ordered locale preferences as ICU4X `Locale`
values, not raw strings.  Platform adapters must parse external locale input
before publishing an environment update; widgets receive the parsed values
through the runtime.

ICU4X uses its `compiled_data` strategy for Incular's direct Unicode services.
That keeps the initial runtime self-contained; custom data providers remain an
advanced future integration point rather than framework infrastructure.

`LocaleResolver` applies ICU4X locale fallback and script directionality to
application-declared `LocalizationCatalog` resources. It also provides
framework-neutral integer/date formatting and ICU cardinal-plural selection
through `PluralForms`. The catalog owns messages and any message-template
syntax; Incular does not impose a translation file format or load application
resources at runtime.

`ApplicationDefaults` and `WidgetDefaults` are the shared default contract.
Window/environment constructors, plain text, and sliver-backed widgets consume
these values rather than maintaining independent copies. A base container does
not acquire a background from this contract: transparent/no-paint is the
neutral widget default, and visible surfaces belong to an explicit root widget
or design-system theme.

`TransparencyMode` is the renderer-independent native presentation intent. It
is intentionally separate from `background_color`: framebuffer alpha determines
how the operating-system compositor treats a window, while the background color
is the base color underneath the rendered scene. Either value can be changed
without implying the other. Platform and renderer crates carry the same typed
transparency value instead of maintaining parallel booleans or renderer-specific
forms.

`WindowSizePolicy` makes native size ownership explicit. `Viewport` keeps the
ordinary desktop contract where native metrics tightly constrain the retained
root. `Content` is opt-in for compact utility/popup-style windows: the declared
initial size remains a stable minimum, the retained root layout can request a
larger native host, and the host shrinks back when primary content becomes
smaller. Paint-only overflow, shadows, filters, and transient portals do not
participate in top-level sizing. Decoration is deliberately unrelated to this
policy; an undecorated window is not implicitly content-sized.

`TransientPresentation` is orthogonal again: `Auto` lets a capable platform
host lift menus/popovers/tooltips into a transient surface and otherwise keeps
them in the owning view's overlay, while `Overlay` explicitly requests the
in-view representation. This policy never implies a top-level resize.
