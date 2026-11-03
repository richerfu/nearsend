# NearSend

A LocalSend protocol-compatible client built with GPUI and GPUI Kit.

## Overview

NearSend is a cross-platform file sharing application that implements the LocalSend protocol v2.0. It allows you to securely share files and messages with nearby devices over your local network without requiring an internet connection.

## GPUI dependencies

The OHOS build targets `gpui-pre 0.3.11`, GPUI Kit components and assets `0.9.0`,
`gpui-ohos 0.0.1-beta.0`, and `gpui-router 0.7.0`.
`gpui-ohos` currently has only a beta release on crates.io. GPUI dependencies
no longer use Git; LocalSend keeps its existing pinned Git revision.

GPUI Kit 0.9.0 supports GPUI Pre 0.3.11 directly. The components, assets,
FPS diagnostics, OHOS backend and router use official crates.io releases.
The OHOS platform is injected directly rather than using Kit's desktop
platform facade.
Routing and navigation history belong to each window. Background events queue
navigation requests, which that window applies through the router's `Navigator`.

## FPS diagnostics

Enable the optional `fps` Cargo feature in a Release build to show the
gpui-fps overlay on the home page. Its frame budget is 8.33 ms (120 Hz);
this budget does not configure the display refresh rate.

The default `MAX` headline estimates CPU drawing throughput. Right-click
the overlay to switch to observed FPS; click it to toggle the compact view.
The overlay is excluded when the feature is disabled.

## Credits

- [LocalSend Protocol](https://github.com/localsend/protocol)
- [LocalSend Application](https://github.com/localsend/localsend)
- [GPUI](https://github.com/zed-industries/zed)
- [GPUI Kit](https://github.com/longbridge/gpui-kit)
- [GPUI-Router](https://github.com/justjavac/gpui-router)

## License

[MIT](./LICENSE)
