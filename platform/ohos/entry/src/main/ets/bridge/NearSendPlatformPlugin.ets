import { Want, wantConstant } from "@kit.AbilityKit";
import { picker } from "@kit.CoreFileKit";
import pasteboard from "@ohos.pasteboard";
import {
  AsyncPluginBase,
  BridgeCallContext,
  BridgeContextRequirement,
  BridgeTypedValue,
} from "@ohos-rs/ability";

const EMPTY_REQUEST_TYPE = "nearsend.platform.EmptyRequest";
const CLIPBOARD_WRITE_REQUEST_TYPE = "nearsend.platform.ClipboardWriteRequest";
const CLIPBOARD_READ_RESPONSE_TYPE = "nearsend.platform.ClipboardReadResponse";
const ACCEPTED_RESPONSE_TYPE = "nearsend.platform.AcceptedResponse";
const SAVE_FILE_REQUEST_TYPE = "nearsend.platform.SaveFileRequest";
const URI_RESPONSE_TYPE = "nearsend.platform.UriResponse";
const OPEN_FILE_REQUEST_TYPE = "nearsend.platform.OpenFileRequest";

interface ClipboardWriteRequest {
  text: string;
}

interface SaveFileRequest {
  fileName: string;
}

interface OpenFileRequest {
  uri: string;
}

class ClipboardReadResponse {
  readonly text: string;

  constructor(text: string) {
    this.text = text;
  }
}

class AcceptedResponse {
  readonly accepted: boolean;

  constructor(accepted: boolean) {
    this.accepted = accepted;
  }
}

class UriResponse {
  readonly uri: string;

  constructor(uri: string) {
    this.uri = uri;
  }
}

function requireEmptyRequest(payload: BridgeTypedValue, action: string): void {
  if (payload.typeName !== EMPTY_REQUEST_TYPE || typeof payload.value !== "object") {
    throw new Error(`${action} requires bridge type ${EMPTY_REQUEST_TYPE}`);
  }
}

function parseClipboardWriteRequest(payload: BridgeTypedValue): ClipboardWriteRequest {
  if (
    payload.typeName !== CLIPBOARD_WRITE_REQUEST_TYPE ||
    typeof payload.value !== "object"
  ) {
    throw new Error(`write-clipboard requires bridge type ${CLIPBOARD_WRITE_REQUEST_TYPE}`);
  }
  const request = payload.value as ClipboardWriteRequest;
  if (typeof request.text !== "string") {
    throw new Error("write-clipboard.text must be a string");
  }
  return request;
}

function parseSaveFileRequest(payload: BridgeTypedValue): SaveFileRequest {
  if (payload.typeName !== SAVE_FILE_REQUEST_TYPE || typeof payload.value !== "object") {
    throw new Error(`pick-save-file requires bridge type ${SAVE_FILE_REQUEST_TYPE}`);
  }
  const request = payload.value as SaveFileRequest;
  if (typeof request.fileName !== "string" || !request.fileName.trim()) {
    throw new Error("pick-save-file.fileName must be non-empty");
  }
  return request;
}

function parseOpenFileRequest(payload: BridgeTypedValue): OpenFileRequest {
  if (payload.typeName !== OPEN_FILE_REQUEST_TYPE || typeof payload.value !== "object") {
    throw new Error(`open-file requires bridge type ${OPEN_FILE_REQUEST_TYPE}`);
  }
  const request = payload.value as OpenFileRequest;
  if (typeof request.uri !== "string" || !request.uri.trim()) {
    throw new Error("open-file.uri must be non-empty");
  }
  return request;
}

function ensureActive(context: BridgeCallContext): void {
  if (!context.isActive()) {
    throw new Error("Ability was destroyed while a NearSend platform call was pending");
  }
}

async function readClipboardText(context: BridgeCallContext): Promise<BridgeTypedValue> {
  try {
    const systemPasteboard = pasteboard.getSystemPasteboard();
    const data = await systemPasteboard.getData();
    ensureActive(context);
    const text = data?.getPrimaryText();
    return {
      typeName: CLIPBOARD_READ_RESPONSE_TYPE,
      value: new ClipboardReadResponse(text ? String(text) : ""),
    };
  } catch (_error) {
    ensureActive(context);
    return {
      typeName: CLIPBOARD_READ_RESPONSE_TYPE,
      value: new ClipboardReadResponse(""),
    };
  }
}

async function writeClipboardText(
  request: ClipboardWriteRequest,
  context: BridgeCallContext,
): Promise<BridgeTypedValue> {
  try {
    const systemPasteboard = pasteboard.getSystemPasteboard();
    const data = pasteboard.createData(pasteboard.MIMETYPE_TEXT_PLAIN, request.text);
    await systemPasteboard.setData(data);
    ensureActive(context);
    return { typeName: ACCEPTED_RESPONSE_TYPE, value: new AcceptedResponse(true) };
  } catch (_error) {
    ensureActive(context);
    return { typeName: ACCEPTED_RESPONSE_TYPE, value: new AcceptedResponse(false) };
  }
}

async function pickSaveFile(
  request: SaveFileRequest,
  context: BridgeCallContext,
): Promise<BridgeTypedValue> {
  try {
    const documentPicker = new picker.DocumentViewPicker();
    const options = new picker.DocumentSaveOptions();
    options.newFileNames = [request.fileName];
    const result = await documentPicker.save(options);
    ensureActive(context);
    return {
      typeName: URI_RESPONSE_TYPE,
      value: new UriResponse(result && result.length > 0 ? result[0] : ""),
    };
  } catch (_error) {
    ensureActive(context);
    return { typeName: URI_RESPONSE_TYPE, value: new UriResponse("") };
  }
}

async function openFile(
  request: OpenFileRequest,
  context: BridgeCallContext,
): Promise<BridgeTypedValue> {
  try {
    const uri = request.uri.endsWith("/") ? request.uri : `${request.uri}/`;
    const want: Want = {
      action: "ohos.want.action.viewData",
      uri,
      flags:
        wantConstant.Flags.FLAG_AUTH_READ_URI_PERMISSION |
        wantConstant.Flags.FLAG_AUTH_WRITE_URI_PERMISSION,
    };
    await context.abilityContext.startAbility(want);
    ensureActive(context);
    return { typeName: ACCEPTED_RESPONSE_TYPE, value: new AcceptedResponse(true) };
  } catch (_error) {
    ensureActive(context);
    return { typeName: ACCEPTED_RESPONSE_TYPE, value: new AcceptedResponse(false) };
  }
}

export class NearSendPlatformPlugin extends AsyncPluginBase {
  readonly id = "nearsend.platform";
  readonly requires: BridgeContextRequirement[] = ["ability"];

  async invokeAsync(
    action: string,
    payload: BridgeTypedValue,
    context: BridgeCallContext,
  ): Promise<BridgeTypedValue> {
    if (!context.isActive()) {
      throw new Error("NearSend platform plugin is not active");
    }

    switch (action) {
      case "read-clipboard":
        requireEmptyRequest(payload, action);
        return await readClipboardText(context);
      case "write-clipboard":
        return await writeClipboardText(parseClipboardWriteRequest(payload), context);
      case "pick-save-file":
        return await pickSaveFile(parseSaveFileRequest(payload), context);
      case "open-file":
        return await openFile(parseOpenFileRequest(payload), context);
      default:
        throw new Error(`Unsupported nearsend.platform action '${action}'`);
    }
  }
}
