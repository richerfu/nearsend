# NearSend

A LocalSend protocol-compatible client built with GPUI and gpui-component.

## Overview

NearSend is a cross-platform file sharing application that implements the LocalSend protocol v2.0. It allows you to securely share files and messages with nearby devices over your local network without requiring an internet connection.

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
- [GPUI-Component](https://github.com/longbridge/gpui-component)
- [GPUI-Router](https://github.com/justjavac/gpui-router)

## License

[MIT](./LICENSE)
