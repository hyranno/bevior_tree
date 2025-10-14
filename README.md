
# `bevior_tree`

[![Crates.io](https://img.shields.io/crates/v/bevior_tree)](https://crates.io/crates/bevior_tree)
[![Doc.rs](https://img.shields.io/docsrs/bevior_tree)](https://docs.rs/bevior_tree/)
[![MIT/Apache 2.0](https://img.shields.io/badge/license-MIT%2FApache-blue.svg)](#license)

`bevior_tree` is behavior tree plugin for Bevy.

See `examples` directory.
The `chase.rs` example is written for your first step.
[Docs](https://docs.rs/bevior_tree/) are available, too.

If you want to know about specific node, unit tests in the code might help.

This crate started with reference to [`seldom_state`](https://github.com/Seldom-SE/seldom_state),
    which is good for state machines.


## Comparison
`bevior_tree` is not the only option for making game ai.
Also you don't have to choose only one.
Choose or combine them for your needs.
For example:
* [`seldom_state`](https://github.com/Seldom-SE/seldom_state) is implementation of state machine.
    Good for things that have rigid states, not limiting to ai.
    No good for lots of interconnected states, since it has too much transitions to add.
* [`big-brain`](https://github.com/zkat/big-brain) is implementation of utility ai.
    Utility ai select next action by their utility (expected gain).
    Perhaps you can use `ForcedSelector` kind in `bevior_tree::sequential` to do similar things.


## Compatibility

| Bevy | `bevior_tree` | 
| ---- | ------------- |
| 0.17 | 0.9           |
| 0.16 | 0.8           |
| 0.15 | 0.7           |
| 0.14 | 0.6           |
| 0.13 | 0.5           |
| 0.12 | 0.4           |
| 0.11 | 0.1 - 0.3     |


## License

`bevior_tree` is dual-licensed under MIT and Apache 2.0 at your option.

## Contributing

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the
work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any
additional terms or conditions.
