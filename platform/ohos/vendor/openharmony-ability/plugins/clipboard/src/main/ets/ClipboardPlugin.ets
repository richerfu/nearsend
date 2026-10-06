import pasteboard from "@ohos.pasteboard";
import { image } from "@kit.ImageKit";
import { BusinessError } from "@kit.BasicServicesKit";
import {
  AsyncPluginBase,
  BridgeCallContext,
  BridgeTypedValue,
  BridgeContextRequirement,
  expectBridgeRequestType,
} from "@ohos-rs/ability";

const READ_TEXT_REQUEST_TYPE = "ohos.clipboard.ReadTextRequest";
const READ_TEXT_RESPONSE_TYPE = "ohos.clipboard.ReadTextResponse";
const READ_CONTENT_RESPONSE_TYPE = "ohos.clipboard.ReadContentResponse";
const WRITE_ENCODED_IMAGE_REQUEST_TYPE = "ohos.clipboard.WriteEncodedImageRequest";
const WRITE_URIS_REQUEST_TYPE = "ohos.clipboard.WriteUrisRequest";
const WRITE_TEXT_REQUEST_TYPE = "ohos.clipboard.WriteTextRequest";
const WRITE_TEXT_RESPONSE_TYPE = "ohos.clipboard.WriteTextResponse";
const WRITE_IMAGE_REQUEST_TYPE = "ohos.clipboard.WriteImageRequest";
const WRITE_IMAGE_RESPONSE_TYPE = "ohos.clipboard.WriteImageResponse";
const WRITE_HTML_REQUEST_TYPE = "ohos.clipboard.WriteHtmlRequest";
const WRITE_HTML_RESPONSE_TYPE = "ohos.clipboard.WriteHtmlResponse";
const CLEAR_REQUEST_TYPE = "ohos.clipboard.ClearRequest";
const CLEAR_RESPONSE_TYPE = "ohos.clipboard.ClearResponse";

interface WriteTextRequest {
  text: string;
}

interface WriteEncodedImageRequest {
  bytes: Uint8Array | number[];
}

interface WriteUrisRequest {
  uris: string[];
}

class ReadContentResponse {
  readonly text?: string;
  readonly png?: number[];
  readonly uris: string[];

  constructor(text: string | undefined, png: number[] | undefined, uris: string[]) {
    this.text = text;
    this.png = png;
    this.uris = uris;
  }
}

interface WriteImageRequest {
  rgba: Uint8Array;
  width: number;
  height: number;
}

interface WriteHtmlRequest {
  html: string;
}

interface ClearRequest {}

class ReadTextResponse {
  readonly text: string;

  constructor(text: string) {
    this.text = text;
  }
}

class WriteTextResponse {
  readonly accepted: boolean;

  constructor(accepted: boolean) {
    this.accepted = accepted;
  }
}

class WriteImageResponse {
  readonly accepted: boolean;

  constructor(accepted: boolean) {
    this.accepted = accepted;
  }
}

class WriteHtmlResponse {
  readonly accepted: boolean;

  constructor(accepted: boolean) {
    this.accepted = accepted;
  }
}

class ClearResponse {
  readonly accepted: boolean;

  constructor(accepted: boolean) {
    this.accepted = accepted;
  }
}

export class ClipboardPlugin extends AsyncPluginBase {
  readonly id = "ohos.clipboard";
  readonly requires: BridgeContextRequirement[] = ["ability"];

  async invokeAsync(
    action: string,
    payload: BridgeTypedValue,
    context: BridgeCallContext,
  ): Promise<BridgeTypedValue> {
    if (!context.isActive()) {
      throw new Error("Bridge session is no longer active");
    }

    if (action === "read-content") {
      expectBridgeRequestType(payload, "ohos.clipboard", action, READ_TEXT_REQUEST_TYPE);
      const systemPasteboard = pasteboard.getSystemPasteboard();
      const data = await systemPasteboard.getData();
      if (data.getRecordCount() === 0 && (await systemPasteboard.hasData())) {
        throw new Error("Clipboard data is unreadable; check READ_PASTEBOARD signing profile ACL");
      }
      const mimeTypes = data.getMimeTypes();
      let text: string | undefined;
      let png: number[] | undefined;
      const uris: string[] = [];
      if (mimeTypes.includes(pasteboard.MIMETYPE_TEXT_PLAIN)) {
        text = data.getPrimaryText() || undefined;
      }
      if (mimeTypes.includes(pasteboard.MIMETYPE_TEXT_URI)) {
        for (let index = 0; index < data.getRecordCount(); index++) {
          const uri = data.getRecordAt(index).uri;
          if (typeof uri === "string" && uri.length > 0) {
            uris.push(uri);
          }
        }
      }
      if (mimeTypes.includes(pasteboard.MIMETYPE_PIXELMAP)) {
        const pixelMap = data.getPrimaryPixelMap();
        if (pixelMap) {
          const packer = image.createImagePacker();
          try {
            const encoded = await packer.packToData(pixelMap, {
              format: "image/png",
              quality: 100,
            });
            png = Array.from(new Uint8Array(encoded));
          } finally {
            packer.release();
          }
        }
      }
      return {
        typeName: READ_CONTENT_RESPONSE_TYPE,
        value: new ReadContentResponse(text, png, uris),
      };
    }

    if (action === "write-encoded-image") {
      const request = expectBridgeRequestType(
        payload,
        "ohos.clipboard",
        action,
        WRITE_ENCODED_IMAGE_REQUEST_TYPE,
      ) as WriteEncodedImageRequest;
      const bytes =
        request.bytes instanceof Uint8Array ? request.bytes : new Uint8Array(request.bytes);
      if (bytes.length === 0) {
        throw new Error("write-encoded-image.bytes must not be empty");
      }
      const copy = new Uint8Array(bytes.length);
      copy.set(bytes);
      const source = image.createImageSource(copy.buffer);
      try {
        const pixelMap = await source.createPixelMap();
        try {
          await pasteboard
            .getSystemPasteboard()
            .setData(pasteboard.createData(pasteboard.MIMETYPE_PIXELMAP, pixelMap));
        } finally {
          pixelMap.release();
        }
      } finally {
        source.release();
      }
      return {
        typeName: WRITE_IMAGE_RESPONSE_TYPE,
        value: new WriteImageResponse(true),
      };
    }

    if (action === "write-uris") {
      const request = expectBridgeRequestType(
        payload,
        "ohos.clipboard",
        action,
        WRITE_URIS_REQUEST_TYPE,
      ) as WriteUrisRequest;
      if (
        !Array.isArray(request.uris) ||
        request.uris.length === 0 ||
        request.uris.some((uri) => typeof uri !== "string" || !uri.startsWith("file://"))
      ) {
        throw new Error("write-uris.uris must contain file:// URIs");
      }
      const data = pasteboard.createData(pasteboard.MIMETYPE_TEXT_URI, request.uris[0]);
      for (let index = 1; index < request.uris.length; index++) {
        data.addRecord(pasteboard.MIMETYPE_TEXT_URI, request.uris[index]);
      }
      await pasteboard.getSystemPasteboard().setData(data);
      return { typeName: WRITE_TEXT_RESPONSE_TYPE, value: new WriteTextResponse(true) };
    }

    if (action === "read-text") {
      expectBridgeRequestType(payload, "ohos.clipboard", action, READ_TEXT_REQUEST_TYPE);
      const systemPasteboard = pasteboard.getSystemPasteboard();
      const data = await systemPasteboard.getData();
      // On OHOS, a denied READ_PASTEBOARD can return an empty PasteData without
      // rejecting getData(). Distinguish that from a genuinely empty clipboard.
      if (data.getRecordCount() === 0 && (await systemPasteboard.hasData())) {
        throw new Error(
          "Clipboard contains data but cannot be read; check ohos.permission.READ_PASTEBOARD and the signing profile ACL",
        );
      }
      let text: string = "";
      try {
        const primaryText = data.getPrimaryText();
        // getPrimaryText is typed string|null but returns `undefined` on an empty board
        // (e.g. when READ_PASTEBOARD permission is missing — getData silently returns an
        // empty PasteData). Guard all three: null, undefined, and non-string.
        if (
          primaryText !== null &&
          primaryText !== undefined &&
          typeof primaryText === "string" &&
          primaryText.length > 0
        ) {
          text = primaryText;
        }
      } catch {
        text = "";
      }
      return {
        typeName: READ_TEXT_RESPONSE_TYPE,
        value: new ReadTextResponse(text),
      };
    }

    if (action === "write-text") {
      const request = expectBridgeRequestType(
        payload,
        "ohos.clipboard",
        action,
        WRITE_TEXT_REQUEST_TYPE,
      ) as WriteTextRequest;
      if (typeof request.text !== "string") {
        throw new Error("write-text.text must be a string");
      }
      const pasteData = pasteboard.createData(pasteboard.MIMETYPE_TEXT_PLAIN, request.text);
      const systemPasteboard = pasteboard.getSystemPasteboard();
      await systemPasteboard.setData(pasteData);
      return {
        typeName: WRITE_TEXT_RESPONSE_TYPE,
        value: new WriteTextResponse(true),
      };
    }

    if (action === "write-image") {
      const request = expectBridgeRequestType(
        payload,
        "ohos.clipboard",
        action,
        WRITE_IMAGE_REQUEST_TYPE,
      ) as WriteImageRequest;
      // NAPI Vec<u8> inside #[napi(object)] arrives as Array<number>, not Uint8Array — convert
      let rgbaData: Uint8Array;
      if (request.rgba instanceof Uint8Array) {
        rgbaData = request.rgba;
      } else if (Array.isArray(request.rgba)) {
        rgbaData = new Uint8Array(request.rgba as number[]);
      } else {
        throw new Error("write-image.rgba must be a Uint8Array or number array");
      }
      if (!Number.isInteger(request.width) || !Number.isInteger(request.height)) {
        throw new Error("write-image.width and height must be integers");
      }
      const expected = request.width * request.height * 4;
      if (rgbaData.length !== expected) {
        throw new Error(
          `write-image.rgba length ${rgbaData.length} != expected ${expected} (${request.width}x${request.height}x4)`,
        );
      }
      await this.writeImageToClipboard(rgbaData, request.width, request.height);
      return {
        typeName: WRITE_IMAGE_RESPONSE_TYPE,
        value: new WriteImageResponse(true),
      };
    }

    if (action === "write-html") {
      const request = expectBridgeRequestType(
        payload,
        "ohos.clipboard",
        action,
        WRITE_HTML_REQUEST_TYPE,
      ) as WriteHtmlRequest;
      if (typeof request.html !== "string") {
        throw new Error("write-html.html must be a string");
      }
      const pasteData = pasteboard.createData(pasteboard.MIMETYPE_TEXT_HTML, request.html);
      const systemPasteboard = pasteboard.getSystemPasteboard();
      await systemPasteboard.setData(pasteData);
      return {
        typeName: WRITE_HTML_RESPONSE_TYPE,
        value: new WriteHtmlResponse(true),
      };
    }

    if (action === "clear") {
      expectBridgeRequestType(payload, "ohos.clipboard", action, CLEAR_REQUEST_TYPE);
      const systemPasteboard = pasteboard.getSystemPasteboard();
      await systemPasteboard.clearData();
      return {
        typeName: CLEAR_RESPONSE_TYPE,
        value: new ClearResponse(true),
      };
    }

    throw new Error(`Unsupported ohos.clipboard action '${action}'`);
  }

  /**
   * Creates a PixelMap from RGBA data and writes it to the system clipboard.
   * Reuses the same logic as ClipboardHelper.ets writeImageToClipboard.
   */
  private async writeImageToClipboard(
    rgbaData: Uint8Array,
    width: number,
    height: number,
  ): Promise<void> {
    let pm: image.PixelMap | null = null;
    try {
      // Copy napi-provided Uint8Array into a standard JS-managed buffer.
      // napi external buffers may not be properly accessible by ArkTS PixelMap APIs.
      const jsArr = new Uint8Array(rgbaData.length);
      jsArr.set(rgbaData);

      const opts: image.InitializationOptions = {
        editable: true,
        pixelFormat: image.PixelMapFormat.RGBA_8888,
        size: { width: width, height: height },
      };
      pm = image.createPixelMapSync(opts);
      pm.writeBufferToPixelsSync(jsArr.buffer);

      const pasteData = pasteboard.createData(pasteboard.MIMETYPE_PIXELMAP, pm);
      const systemPasteboard = pasteboard.getSystemPasteboard();
      await systemPasteboard.setData(pasteData);
    } catch (e) {
      const err = e as BusinessError;
      console.error(`[Clipboard] writeImageToClipboard failed: ${err.code} ${err.message}`);
      throw e as Error;
    } finally {
      pm?.release();
    }
  }
}
