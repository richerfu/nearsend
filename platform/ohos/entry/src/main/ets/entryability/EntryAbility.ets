import window from "@ohos.window";
import { AbilityConstant, Want } from "@kit.AbilityKit";
import { systemShare } from "@kit.ShareKit";
import { LazyPlugin, NativeAbility } from "@ohos-rs/ability";
import { AppControlPlugin } from "@ohos-rs/ability-plugin-app-control";
import { FilesPlugin } from "@ohos-rs/ability-plugin-files";
import { PermissionPlugin } from "@ohos-rs/ability-plugin-permission";
import * as nearSendNative from "libnear_send.so";
import { NearSendPlatformPlugin } from "../bridge/NearSendPlatformPlugin";

const NATIVE_MODULE_NAME = "near_send";
const SHARE_ACTIONS: string[] = [
  "ohos.want.action.sendData",
  "ohos.want.action.sendMultipleData",
];
const PARAMS_STREAM = "ability.params.stream";
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

  override onCreate(
    want: Want,
    launchParam: AbilityConstant.LaunchParam,
  ): void {
    super.onCreate(want, launchParam);
    this.handleShareWant(want);
  }

  override onNewWant(
    want: Want,
    launchParam: AbilityConstant.LaunchParam,
  ): void {
    super.onNewWant(want, launchParam);
    this.handleShareWant(want);
  }

  private handleShareWant(want: Want): void {
    if (!SHARE_ACTIONS.includes(want.action ?? "")) {
      return;
    }
    void this.forwardSharedContent(want);
  }

  private async forwardSharedContent(want: Want): Promise<void> {
    const uris: string[] = [];
    const texts: string[] = [];
    try {
      const sharedData = await systemShare.getSharedData(want);
      for (const record of sharedData.getRecords()) {
        if (typeof record.uri === "string" && record.uri.trim().length > 0) {
          uris.push(record.uri);
        }
        if (typeof record.content === "string" && record.content.trim().length > 0) {
          texts.push(record.content);
        }
      }
    } catch (error) {
      console.warn(`[NearSend] Share Kit could not parse incoming Want: ${String(error)}`);
    }

    // Keep compatibility with applications that still construct a legacy
    // sendData Want instead of using Share Kit.
    this.appendUri(want.uri, uris);
    const stream = want.parameters?.[PARAMS_STREAM];
    if (typeof stream === "string") {
      this.appendUri(stream, uris);
    } else if (Array.isArray(stream)) {
      const streamValues = stream as Array<Object>;
      for (const value of streamValues) {
        if (typeof value === "string") {
          this.appendUri(value, uris);
        }
      }
    }

    nearSendNative.enqueueSystemShare(uris, texts);
  }

  private appendUri(value: string | undefined, uris: string[]): void {
    if (typeof value === "string" && value.trim().length > 0 && !uris.includes(value)) {
      uris.push(value);
    }
  }

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
