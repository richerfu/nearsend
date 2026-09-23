# NearSend

A LocalSend protocol-compatible client built with GPUI and gpui-component.

## Overview

NearSend is a cross-platform file sharing application that implements the LocalSend protocol v2.0. It allows you to securely share files and messages with nearby devices over your local network without requiring an internet connection.

## OpenHarmony build

The OpenHarmony project uses the `openharmony-ability` fork pinned as a Git submodule. After cloning this branch, run:

```sh
git submodule update --init platform/ohos/vendor/openharmony-ability
ohrs build --arch arm64
cp target/aarch64-unknown-linux-ohos/debug/libnear_send.so platform/ohos/entry/libs/arm64-v8a/libnear_send.so
cd platform/ohos
ohpm install --all
hvigorw assembleHap --mode module -p product=default
```

The signed package is generated at `platform/ohos/entry/build/default/outputs/default/entry-default-signed.hap` when a signing configuration is available.

## Credits

- [LocalSend Protocol](https://github.com/localsend/protocol)
- [LocalSend Application](https://github.com/localsend/localsend)
- [GPUI](https://github.com/zed-industries/zed)
- [GPUI-Component](https://github.com/longbridge/gpui-component)
- [GPUI-Router](https://github.com/justjavac/gpui-router)

## License

[MIT](./LICENSE)
