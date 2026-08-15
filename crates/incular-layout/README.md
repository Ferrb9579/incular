# incular-layout

Renderer-independent layout primitives and algorithms for Incular, including
constraints, sizes, positions, alignment, and future layout containers.
# incular-layout

Owns Flutter-style `Constraints`, `EdgeInsets`, axes, and alignment values. It
depends only on `incular-core`; widgets choose layout algorithms. Constraints
are validated on construction and permit infinity only as an upper bound.
