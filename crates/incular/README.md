# incular

The public facade crate for Incular applications. It will re-export the stable
public API from the framework layers and expose desktop support through the
`desktop` feature.

The default facade feature includes `material`, so application code can use
Flutter-shaped controls from `incular::prelude` or `incular::material`. Disable
that feature for a renderer-neutral/core-only integration; core interactions
are composed with `GestureDetector` and editors use `EditableText`.
