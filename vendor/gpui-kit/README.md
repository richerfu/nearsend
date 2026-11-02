# GPUI Kit 0.8.1 snapshot alignment

These four crates come from the crates.io 0.8.1 release archives. Their
normalized `Cargo.toml` files pin the `gpui-pre-*` crates to `=0.3.11` instead
of `=0.3.10`.

`gpui-ohos 0.0.1-beta.0` requires GPUI 0.3.11. Cargo cannot resolve that with
GPUI Kit 0.8.1's exact 0.3.10 pins, and using different GPUI implementations
would give the UI and platform incompatible types. GPUI 0.3.11 also changed
`WindowTextSystem` from `Arc` to `Rc`. In `gpui-base/src/text/node.rs`, the
Markdown table width cache therefore keeps a weak shared `TextSystem` and a
generational `WindowId`, preserving cache isolation and allowing parsed
documents to cross threads without retaining a window's text layout state.
All other Rust sources and assets are unchanged. Remove these patches when
GPUI Kit publishes a release with matching snapshot pins and this API change.

NearSend uses Kit's component and assets crates directly. The `gpui-kit`
facade currently selects the desktop Linux platform on OHOS; NearSend supplies
its own platform through `gpui-ohos` instead.

Original archive URLs and SHA-256 hashes:

| Crate | Archive SHA-256 |
| --- | --- |
| [gpui-base](https://static.crates.io/crates/gpui-base/gpui-base-0.8.1.crate) | `d862bc8f2e27710039013d075ac4995892e9687ff207fa530f880cbcd513936f` |
| [gpui-component](https://static.crates.io/crates/gpui-component/gpui-component-0.8.1.crate) | `46ce53430332e30f303364eaabfa10ed2aa23dfaf2a147e04370e0e9c423578b` |
| [gpui-kit-assets](https://static.crates.io/crates/gpui-kit-assets/gpui-kit-assets-0.8.1.crate) | `c214017b45e5654665f18faa551f58a2c2c8d25efe6632b1a443ba8edf365e96` |
| [gpui-fps](https://static.crates.io/crates/gpui-fps/gpui-fps-0.8.1.crate) | `5e6d53d04266a5236c99a740859af8b326a9b8632f4ef95ed3acd184e22c76cb` |

The original release metadata and included license files are preserved.
