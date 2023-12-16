# Tom's Maid

Keep your TOML files clean thanks to Tom's maid.

This formatter tries to apply an opinionated consistent formatting style.

Mainly, it considers lines not separated by blank lines as blocks, such that
sorting is only applied inside each blocks. It matches the practice in some
big Rust repositories to separate dependencies in sections, when many
other formatters don't take that into account and scramble the sections.