# NearSend ArkTS dependency snapshot

Source: `https://github.com/richerfu/openharmony-ability.git`, revision
`eab9d1192d61e36bf14f17d8180c6e71d9f87b3b` (branch
`feat/ohos-adaptation-runtime-fixes`). This is the submodule revision pinned by
the `gpui-ohos` git revision in the root Cargo manifest, which also supplies the
matching openharmony-ability Rust crates.

The `native_ability` module and the app-control, clipboard, files, menu,
notification, permission, process, system-state, url and window plugins are
copied from that revision without local changes. The files plugin already lets
the system picker decide folder support, so capable tablets can select folders.
When bumping `gpui-ohos`, refresh this snapshot from its submodule revision.

Hvigor regenerates each module's `BuildProfile.ets` while building; the committed
copy remains the upstream version. Root license files also serve the module
license symlinks.

Local signing material remains in the developer's `build-profile.json5` and
ignored key directories. Only module registration changes are committed from
the top-level build profile.
