import window from "@ohos.window";
import { LazyPlugin, NativeAbility } from "@ohos-rs/ability";
import { AppControlPlugin } from "@ohos-rs/ability-plugin-app-control";
import { FilesPlugin } from "@ohos-rs/ability-plugin-files";
import { PermissionPlugin } from "@ohos-rs/ability-plugin-permission";
import { NearSendPlatformPlugin } from "../bridge/NearSendPlatformPlugin";

const NATIVE_MODULE_NAME = "near_send";

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
      await windowStage.loadContent("pages/Index");
    } catch (error) {
      throw new Error(`Unable to load Nearsend WindowStage content: ${String(error)}`);
    }
  }
}
