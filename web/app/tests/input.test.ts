import assert from "node:assert/strict";
import { test } from "node:test";
import { InputController } from "../input.ts";
import type { AppServices, AppState, GameIntent, RenderCamera, RendererHandle, UiHandle } from "../../shared/contracts.ts";
import type { SourceCameraControls } from "../manifest.ts";
import type { UiWorldAdapter } from "../ui-adapter.ts";
import type { NativeCameraInput, NativeCameraInputSink } from "../camera.ts";

class Surface extends EventTarget {
  width = 1920; height = 1080; captured = new Set<number>();
  getBoundingClientRect() { return { left: 0, top: 0, width: 1920, height: 1080 }; }
  setPointerCapture(id: number) { this.captured.add(id); }
  releasePointerCapture(id: number) { this.captured.delete(id); }
  hasPointerCapture(id: number) { return this.captured.has(id); }
}
function pointer(type: string, fields: Record<string, unknown> = {}): Event {
  const event = new Event(type, { cancelable: true });
  Object.assign(event, { pointerId: 1, clientX: 500, clientY: 400, button: 0, ctrlKey: false, ...fields });
  return event;
}

test("test-only input surface honors source UI capture, menus, drags, and editable focus before picking", async () => {
  const doc = Object.assign(new EventTarget(), { activeElement: null as EventTarget | null });
  const win = new EventTarget();
  const oldDocument = Object.getOwnPropertyDescriptor(globalThis, "document");
  const oldWindow = Object.getOwnPropertyDescriptor(globalThis, "window");
  Object.defineProperty(globalThis, "document", { value: doc, configurable: true });
  Object.defineProperty(globalThis, "window", { value: win, configurable: true });
  const surface = new Surface();
  let capturedByUi = false;
  let contextMenus = 0;
  const picks: Array<[number, number]> = [];
  const intents: GameIntent[] = [];
  const cameras: RenderCamera[] = [];
  const uiCameras: RenderCamera[] = [];
  let requestCamera: (yaw: number) => void = () => {};
  const state = {
    phase: "world", world: { player: { tile: { x: 3200, y: 3200, plane: 0 }, settings: [{ setting: "run", enabled: false }] } },
  } as AppState;
  const services = {
    state: () => state,
    send: async (intent: GameIntent) => { intents.push(intent); },
    report: () => {},
  } as unknown as AppServices;
  const ui = {
    capturesPointer: () => capturedByUi,
  } as unknown as UiHandle;
  const renderer = {
    camera: (camera: RenderCamera) => { cameras.push({ ...camera }); },
    pick: (x: number, y: number) => { picks.push([x, y]); return { kind: "tile", tile: { x: 3201, y: 3200, plane: 0 } }; },
  } as unknown as RendererHandle;
  const controls: SourceCameraControls = {
    yawUnitsPerPixel: 8, pitchUnitsPerPixel: 8, keyboardYawUnitsPerSecond: 200,
    keyboardPitchUnitsPerSecond: 100, minimumPitch: 1000, maximumPitch: 5000,
    zoomPerWheelStep: 10, minimumZoom: 100, maximumZoom: 1000, tileWorldUnits: 128,
  };
  const camera: RenderCamera = { x: 1, y: 2, height: 3, pitch: 2000, yaw: 0, unitsPerTurn: 16384, zoom: 500, near: 1, far: 5000 };
  const adapter: UiWorldAdapter = {
    pointer(_ui, event) {
      if (event.kind === "context") contextMenus++;
      if (event.kind === "primary" && event.pick?.kind === "tile") {
        void services.send({ kind: "walk", destination: event.pick.tile, running: Boolean(event.control) });
      }
      assert.equal(event.x, 500);
      assert.equal(event.y, 400);
      return true;
    },
    camera(_ui, camera) { uiCameras.push({ ...camera }); },
    cameraRequests(_ui, listener) { requestCamera = listener; return () => {}; },
  };
  const input = new InputController(surface as unknown as HTMLCanvasElement, ui, renderer, services, camera, controls,
    { x: 3200, y: 3200, plane: 0 }, () => ({ width: 3840, height: 2160 }), adapter);
  const click = (button = 0) => { surface.dispatchEvent(pointer("pointerdown", { button })); surface.dispatchEvent(pointer("pointerup", { button })); };
  try {
    click();
    assert.deepEqual(intents, [{ kind: "walk", destination: { x: 3201, y: 3200, plane: 0 }, running: false }]);
    assert.deepEqual(picks[0], [1000, 800], "Only renderer picking uses backing-pixel coordinates.");
    capturedByUi = true; click();
    capturedByUi = false;
    click(2); surface.dispatchEvent(pointer("contextmenu", { button: 2 }));
    assert.equal(intents.length, 1, "right click creates no world action");
    assert.equal(contextMenus, 1);
    capturedByUi = true;
    surface.dispatchEvent(pointer("pointerdown"));
    capturedByUi = false;
    surface.dispatchEvent(pointer("pointerup"));
    assert.equal(intents.length, 1, "a UI press cannot become a world release");
    surface.dispatchEvent(pointer("pointerdown"));
    surface.dispatchEvent(pointer("pointerup", { clientX: 800 }));
    assert.equal(intents.length, 1, "drag release is not a click");
    doc.activeElement = { tagName: "INPUT" } as unknown as EventTarget;
    click();
    doc.dispatchEvent(pointer("keydown", { code: "ArrowRight" }));
    input.update(16);
    assert.equal(intents.length, 1);
    assert.equal(cameras.length, 1, "typing focus does not move the camera");
    doc.activeElement = null;
    surface.dispatchEvent(pointer("pointerdown", { button: 1 }));
    surface.dispatchEvent(pointer("pointermove", { button: 1, clientX: 510, clientY: 420 }));
    surface.dispatchEvent(pointer("pointerup", { button: 1 }));
    assert.equal(intents.length, 1);
    assert.equal(cameras.at(-1)?.yaw, 80);
    assert.equal(cameras.at(-1)?.pitch, 2160);
    surface.dispatchEvent(pointer("wheel", { deltaY: 100 }));
    assert.equal(cameras.at(-1)?.zoom, 510);
    doc.dispatchEvent(pointer("keydown", { code: "ArrowRight" }));
    input.update(100);
    assert.equal(cameras.at(-1)?.yaw, 100);
    doc.dispatchEvent(pointer("keyup", { code: "ArrowRight" }));
    requestCamera(0);
    assert.equal(cameras.at(-1)?.yaw, 0);
    assert.deepEqual(uiCameras.at(-1), cameras.at(-1), "UI and renderer observe the same actual camera.");
    const cancelled = pointer("pointerdown");
    cancelled.preventDefault();
    surface.dispatchEvent(cancelled);
    surface.dispatchEvent(pointer("pointerup"));
    assert.equal(intents.length, 1, "UI preventDefault always wins");
  } finally {
    input.dispose();
    if (oldDocument) Object.defineProperty(globalThis, "document", oldDocument);
    else Reflect.deleteProperty(globalThis, "document");
    if (oldWindow) Object.defineProperty(globalThis, "window", oldWindow);
    else Reflect.deleteProperty(globalThis, "window");
  }
});

test("native input transport owns logical pixels, held keys, middle capture, wheel routing and teardown without TS camera mechanics", () => {
  const doc = Object.assign(new EventTarget(), { activeElement: null as EventTarget | null, hidden: false });
  const win = new EventTarget();
  const oldDocument = Object.getOwnPropertyDescriptor(globalThis, "document");
  const oldWindow = Object.getOwnPropertyDescriptor(globalThis, "window");
  Object.defineProperty(globalThis, "document", { value: doc, configurable: true });
  Object.defineProperty(globalThis, "window", { value: win, configurable: true });
  const surface = new Surface();
  let uiCapture = false;
  let active = true;
  let phase = "world";
  const inputs: Array<{ at: bigint; input: NativeCameraInput }> = [];
  const yaws: number[] = [];
  const errors: Error[] = [];
  const scalarCameras: RenderCamera[] = [];
  const uiCameras: RenderCamera[] = [];
  let compass = (_yaw: number): void => {};
  const sink: NativeCameraInputSink = {
    ready: () => active,
    input(at, input) { inputs.push({ at, input }); },
    faceYaw(yaw) { yaws.push(yaw); },
  };
  const renderer = { camera: (value: RenderCamera) => { scalarCameras.push(value); }, pick: () => null } as unknown as RendererHandle;
  const services = { state: () => ({ phase }), report: (error: Error) => { errors.push(error); } } as unknown as AppServices;
  const ui = { capturesPointer: () => uiCapture } as unknown as UiHandle;
  const adapter: UiWorldAdapter = {
    pointer: () => true,
    camera(_handle, value) { uiCameras.push(value); },
    cameraRequests(_handle, callback) { compass = callback; return () => { compass = () => {}; }; },
  };
  const camera: RenderCamera = { x: 1, y: 2, height: 3, pitch: 2048, yaw: 0, unitsPerTurn: 16384, zoom: 662, near: 50, far: 3500 };
  const input = new InputController(surface as unknown as HTMLCanvasElement, ui, renderer, services, camera, null,
    { x: 3200, y: 3200, plane: 0 }, () => ({ width: 3840, height: 2160 }), adapter, sink);
  try {
    assert.deepEqual(scalarCameras, [], "normal entry never calls the recorded scalar-camera setter");
    doc.dispatchEvent(pointer("keydown", { code: "ArrowLeft" }));
    doc.dispatchEvent(pointer("keydown", { code: "ArrowLeft", repeat: true }));
    doc.dispatchEvent(pointer("keydown", { code: "ArrowRight" }));
    assert.equal(inputs.length, 2, "key repeat cannot become camera acceleration");
    assert(inputs.at(-1)!.input.arrows.left && inputs.at(-1)!.input.arrows.right, "precedence belongs to Rust");
    const count = inputs.length;
    input.update(1000);
    assert.equal(inputs.length, count, "RAF elapsed time is not a TypeScript input-rate multiplier");
    surface.dispatchEvent(pointer("pointerdown", { button: 1 }));
    surface.dispatchEvent(pointer("pointermove", { button: 1, clientX: 510, clientY: 420 }));
    assert.deepEqual(inputs.at(-1)!.input.mouse, [510, 420], "source logical pixels, not backing pixels or movementX/Y");
    assert.equal(inputs.at(-1)!.input.button, "Middle");
    assert(surface.hasPointerCapture(1));
    surface.dispatchEvent(pointer("pointerup", { button: 1 }));
    assert.equal(inputs.at(-1)!.input.button, "Released");
    assert(!surface.hasPointerCapture(1));
    surface.dispatchEvent(pointer("wheel", { deltaY: 240, deltaMode: 0 }));
    assert.deepEqual(inputs.at(-1)!.input.wheel, { rotation: 1, route: "Camera" });
    const wheelCount = inputs.length;
    uiCapture = true;
    surface.dispatchEvent(pointer("wheel", { deltaY: 120, deltaMode: 0 }));
    surface.dispatchEvent(pointer("pointerdown", { button: 1 }));
    assert.equal(inputs.length, wheelCount, "source UI owns sidebar wheels and middle presses");
    uiCapture = false;
    doc.activeElement = { tagName: "INPUT" } as unknown as EventTarget;
    input.update(16);
    assert.deepEqual(inputs.at(-1)!.input.arrows, { left: false, right: false, up: false, down: false });
    const typing = inputs.length;
    doc.dispatchEvent(pointer("keydown", { code: "ArrowRight" }));
    surface.dispatchEvent(pointer("wheel", { deltaY: 1, deltaMode: 1 }));
    assert.equal(inputs.length, typing);
    doc.activeElement = null;
    compass(1234);
    assert.deepEqual(yaws, [1234]);
    input.publishNativeCamera({ ...camera, yaw: 1234 });
    assert.equal(uiCameras.at(-1)?.yaw, 1234);
    assert.deepEqual(scalarCameras, []);
    assert.throws(() => input.resizeZoom(410), /Rust viewport\/FOV/);
    surface.dispatchEvent(pointer("pointerdown", { button: 1 }));
    win.dispatchEvent(new Event("blur"));
    assert.equal(inputs.at(-1)!.input.button, "Released");
    assert(!surface.hasPointerCapture(1));
    doc.dispatchEvent(pointer("keydown", { code: "ArrowDown" }));
    phase = "reconnecting";
    input.update(20);
    assert.equal(inputs.at(-1)!.input.arrows.down, false);
    active = false;
    input.dispose();
    const disposed = inputs.length;
    surface.dispatchEvent(pointer("pointerdown", { button: 1 }));
    doc.dispatchEvent(pointer("keydown", { code: "ArrowLeft" }));
    assert.equal(inputs.length, disposed);
    assert(inputs.every((event) => typeof event.at === "bigint"));
    assert.deepEqual(errors, []);
  } finally {
    input.dispose();
    if (oldDocument) Object.defineProperty(globalThis, "document", oldDocument);
    else Reflect.deleteProperty(globalThis, "document");
    if (oldWindow) Object.defineProperty(globalThis, "window", oldWindow);
    else Reflect.deleteProperty(globalThis, "window");
  }
});
