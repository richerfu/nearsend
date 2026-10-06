# NearSend ArkTS dependency snapshot

Source: `https://github.com/richerfu/openharmony-ability.git`, branch
`feat/pr82-nearsend-integration`, snapshot revision
`4493821eb1234af66afa4b6571e9fb6558fea4cc`.

The `native_ability` module and the app-control, clipboard, files, menu,
permission, process, url and window plugins are copied from that revision.
The matching Rust crates use the same git branch in the root Cargo manifest
and in `vendor/gpui-ohos/Cargo.toml`.

The source snapshot has no NearSend runtime patches. Hvigor regenerates each
module's `BuildProfile.ets` while building; the committed copy remains the
upstream version. Root license files also serve the module license symlinks.

Local signing material remains in the developer's `build-profile.json5` and
ignored key directories. Only module registration changes are committed from
the top-level build profile.
