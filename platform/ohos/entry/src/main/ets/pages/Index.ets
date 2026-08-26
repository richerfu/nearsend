import { DefaultXComponent, invokeNativeBackPressInterceptor } from "@ohos-rs/ability";

const NATIVE_MODULE_NAME = "near_send";

@Entry
@Component
struct Index {
  onBackPress(): boolean {
    return invokeNativeBackPressInterceptor(NATIVE_MODULE_NAME);
  }

  build() {
    RelativeContainer() {
      DefaultXComponent({ moduleName: NATIVE_MODULE_NAME });
    }
    .height("100%")
    .width("100%");
  }
}
