# incular-config

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
