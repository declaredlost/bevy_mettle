# bevy_mettle_macros

[![Build Status][action-badge]][action-url]
[![Crate Docs][docs-badge]][docs-url]
[![Crate Version][crates-badge]][crates-url]

**Part of [bevy_mettle](https://github.com/declaredlost/bevy_mettle), a gameplay framework for Bevy.**

[action-badge]: https://img.shields.io/github/actions/workflow/status/declaredlost/bevy_mettle/build.yml?branch=main&label=build&logo=github&style=flat-square
[action-url]: https://github.com/declaredlost/bevy_mettle/actions/workflows/build.yml
[crates-badge]: https://img.shields.io/crates/v/bevy_mettle_macros.svg?logo=rust&style=flat-square
[crates-url]: https://crates.io/crates/bevy_mettle_macros
[docs-badge]: https://img.shields.io/docsrs/bevy_mettle_macros?logo=Docs.rs&style=flat-square
[docs-url]: https://docs.rs/bevy_mettle_macros

The procedural macros of [bevy_mettle](https://github.com/declaredlost/bevy_mettle).
Don't depend on it directly: each macro is re-exported by its part.

## Overview

| Macro | Re-exported by |
|---|---|
| `#[derive(Kind)]` | `bevy_mettle_core::registry` |
| `#[derive(Tag)]` | `bevy_mettle_core`, and the `bevy_mettle` facade's prelude |
| `#[derive(Attribute)]` | `bevy_mettle_attribute`, and the facade's prelude |
| `#[derive(Effect)]`, `#[derive(Route)]`, `#[derive(Execution)]`, `#[derive(Calculation)]` (with what they capture) | `bevy_mettle_effect`, and the facade's prelude |
| `#[derive(Ability)]`, `#[derive(Behaviour)]` | `bevy_mettle_ability`, and the facade's prelude |

## Learn more

- The book's [Attributes](https://declaredlost.github.io/bevy_mettle/attributes.html)
  page, and [Feature flags](https://declaredlost.github.io/bevy_mettle/features.html)
- The [API documentation](https://docs.rs/bevy_mettle_macros)

## Contributing

See [CONTRIBUTING.md](https://github.com/declaredlost/bevy_mettle/blob/main/CONTRIBUTING.md)
for development setup and guidelines.

## License

Licensed under either of [Apache License, Version 2.0](https://github.com/declaredlost/bevy_mettle/blob/main/LICENSE-APACHE)
or [MIT license](https://github.com/declaredlost/bevy_mettle/blob/main/LICENSE-MIT) at your option.
