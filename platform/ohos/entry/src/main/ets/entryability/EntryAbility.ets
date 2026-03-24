import { AbilityConstant, Want, wantConstant } from "@kit.AbilityKit";
import { picker } from "@kit.CoreFileKit";
import pasteboard from "@ohos.pasteboard";
import window from "@ohos.window";
import { RustAbility } from "@ohos-rs/ability";
import nearSend from "libnear_send.so";

export default class EntryAbility extends RustAbility {
  public moduleName: string = "near_send";
  public defaultPage: boolean = false;

  private async readClipboardText(): Promise<string> {
    try {
      const systemPasteboard = pasteboard.getSystemPasteboard();
      const data = await systemPasteboard.getData();
      const text = data?.getPrimaryText();
      return text ? String(text) : "";
    } catch (_err) {
      return "";
    }
  }

  private async writeClipboardText(text: string): Promise<boolean> {
    try {
      const systemPasteboard = pasteboard.getSystemPasteboard();
      const data = pasteboard.createData(pasteboard.MIMETYPE_TEXT_PLAIN, text);
      await systemPasteboard.setData(data);
      return true;
    } catch (_err) {
      return false;
    }
  }

  private async pickFilesFromSystem(): Promise<Array<string>> {
    try {
      const documentPicker = new picker.DocumentViewPicker();
      const options = new picker.DocumentSelectOptions();
      options.maxSelectNumber = 500;
      const result = await documentPicker.select(options);
      if (!result || result.length === 0) {
        return [];
      }
      return result;
    } catch (_err) {
      return [];
    }
  }

  private async pickFoldersFromSystem(): Promise<Array<string>> {
    try {
      const documentPicker = new picker.DocumentViewPicker();
      const options = new picker.DocumentSelectOptions();
      options.maxSelectNumber = 8;
      options.selectMode = picker.DocumentSelectMode.FOLDER;
      const result = await documentPicker.select(options);
      if (!result || result.length === 0) {
        return [];
      }
      return result;
    } catch (_err) {
      return [];
    }
  }

  private async pickSaveDirectoryFromSystem(): Promise<string> {
    try {
      const documentPicker = new picker.DocumentViewPicker();
      const options = new picker.DocumentSelectOptions();
      options.maxSelectNumber = 1;
      options.authMode = true;
      options.selectMode = picker.DocumentSelectMode.FOLDER;
      const result = await documentPicker.select(options);
      if (!result || result.length === 0) {
        return "";
      }
      return result[0];
    } catch (_err) {
      return "";
    }
  }

  private async pickSaveFileFromSystem(fileName: string): Promise<string> {
    try {
      const documentPicker = new picker.DocumentViewPicker();
      const options = new picker.DocumentSaveOptions();
      options.newFileNames = [fileName];
      const result = await documentPicker.save(options);
      if (!result || result.length === 0) {
        return "";
      }
      return result[0];
    } catch (_err) {
      return "";
    }
  }

  private async openSavedFileFromSystem(uri: string): Promise<boolean> {
    if (!uri) {
      return false;
    }
    try {
      const want: Want = {
        action: "ohos.want.action.viewData",
        uri: `${uri}/`,
        flags:
          wantConstant.Flags.FLAG_AUTH_READ_URI_PERMISSION |
          wantConstant.Flags.FLAG_AUTH_WRITE_URI_PERMISSION,
      };
      await this.context.startAbility(want);
      return true;
    } catch (_err) {
      return false;
    }
  }

  public async onCreate(want: Want, launchParam: AbilityConstant.LaunchParam): Promise<void> {
    const nearSendBridge = nearSend;
    const setPreferencesPathFn = nearSendBridge.setPreferencesPath;
    const preferencesDir = this.context?.preferencesDir;
    if (
      typeof setPreferencesPathFn === "function" &&
      typeof preferencesDir === "string" &&
      preferencesDir.length > 0
    ) {
      setPreferencesPathFn(preferencesDir);
    }

    const registerFn = nearSendBridge.registerClipboardCallbacks;
    const registerPickerFn = nearSendBridge.registerFilePickerCallbacks;
    const registerSaveFileFn = nearSendBridge.registerSaveFileCallback;
    const registerOpenFileFn = nearSendBridge.registerOpenFileCallback;
    if (typeof registerPickerFn === "function") {
      registerPickerFn(
        async () => await this.pickFilesFromSystem(),
        async () => await this.pickFoldersFromSystem(),
        async () => await this.pickSaveDirectoryFromSystem(),
      );
    }
    if (typeof registerFn === "function") {
      registerFn(
        async () => await this.readClipboardText(),
        async (text: string) => await this.writeClipboardText(text),
      );
    }
    if (typeof registerSaveFileFn === "function") {
      registerSaveFileFn(async (fileName: string) => await this.pickSaveFileFromSystem(fileName));
    }
    if (typeof registerOpenFileFn === "function") {
      registerOpenFileFn(async (uri: string) => await this.openSavedFileFromSystem(uri));
    }
    await super.onCreate(want, launchParam);
  }

  public async onWindowStageCreate(windowStage: window.WindowStage): Promise<void> {
    const win = await windowStage.getMainWindow();
    await super.onWindowStageCreate(windowStage);
    await win.setUIContent("pages/Index");
  }
}
