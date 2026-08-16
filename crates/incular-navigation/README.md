# incular-navigation

Stack navigation, deep-link routing, route transitions, and overlay state for
Incular.

This crate depends one-way on `incular-widgets` for widget descriptions and
retained transition layers. Widgets do not depend on navigation: applications
can use regular widget trees without a route stack, while the public facade
reexports the navigation API for the conventional application-facing surface.
