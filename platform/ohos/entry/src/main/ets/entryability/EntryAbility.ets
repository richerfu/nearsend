import window from "@ohos.window";
import { LazyPlugin, NativeAbility } from "@ohos-rs/ability";
import { AppControlPlugin } from "@ohos-rs/ability-plugin-app-control";
import { FilesPlugin } from "@ohos-rs/ability-plugin-files";
import { PermissionPlugin } from "@ohos-rs/ability-plugin-permission";
import { NearSendPlatformPlugin } from "../bridge/NearSendPlatformPlugin";

const NATIVE_MODULE_NAME = "near_send";
const SAFE_AREA_TYPES: window.AvoidAreaType[] = [
  window.AvoidAreaType.TYPE_SYSTEM,
  window.AvoidAreaType.TYPE_CUTOUT,
  window.AvoidAreaType.TYPE_SYSTEM_GESTURE,
  window.AvoidAreaType.TYPE_NAVIGATION_INDICATOR,
];

export default class EntryAbility extends NativeAbility {
  public moduleName: string = NATIVE_MODULE_NAME;
  public defaultPage: boolean = false;
  public bridgePlugins = [
    new LazyPlugin(() => new PermissionPlugin()),
    new LazyPlugin(() => new AppControlPlugin()),
    new LazyPlugin(() => new FilesPlugin()),
    new LazyPlugin(() => new NearSendPlatformPlugin()),
  ];

  protected override createInitContext(moduleName: string) {
    const context = super.createInitContext(moduleName);
    const preferencesDir = this.context?.preferencesDir;
    if (typeof preferencesDir === "string" && preferencesDir.length > 0) {
      context.prefPath = preferencesDir;
    }
    return context;
  }

  protected override async loadWindowStageContent(
    windowStage: window.WindowStage,
  ): Promise<void> {
    try {
      const mainWindow = windowStage.getMainWindowSync();
      // Let GPUI paint behind the system bars. Page content is inset centrally
      // by AppRoot instead of relying on ArkUI to offset every XComponent page.
      await mainWindow.setWindowLayoutFullScreen(true);

      // The framework subscribes to later avoid-area changes when the
      // XComponent is attached. Seed the current values before its first frame
      // so GPUI can lay out the initial page with the correct safe area too.
      for (const type of SAFE_AREA_TYPES) {
        try {
          const area = mainWindow.getWindowAvoidArea(type);
          this.forEachLifecycle((lifecycle) =>
            lifecycle.windowStageEventCallback.onAvoidAreaChange({ type, area }),
          );
        } catch (_error) {}
      }

      await windowStage.loadContent("pages/Index");
    } catch (error) {
      throw new Error(`Unable to load Nearsend WindowStage content: ${String(error)}`);
    }
  }
}
