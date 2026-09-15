import assert from "node:assert/strict";
import { test } from "node:test";
import { InputController } from "../input.ts";
import type { AppServices, AppState, GameIntent, RenderCamera, RendererHandle, UiHandle } from "../../shared/contracts.ts";
import type { SourceCameraControls } from "../manifest.ts";

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
  const intents: GameIntent[] = [];
  const cameras: RenderCamera[] = [];
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
    worldContext: () => { contextMenus++; },
  } as unknown as UiHandle;
  const renderer = {
    camera: (camera: RenderCamera) => { cameras.push({ ...camera }); },
    pick: () => ({ kind: "tile", tile: { x: 3201, y: 3200, plane: 0 } }),
  } as unknown as RendererHandle;
  const controls: SourceCameraControls = {
    yawUnitsPerPixel: 8, pitchUnitsPerPixel: 8, keyboardYawUnitsPerSecond: 200,
    keyboardPitchUnitsPerSecond: 100, minimumPitch: 1000, maximumPitch: 5000,
    zoomPerWheelStep: 10, minimumZoom: 100, maximumZoom: 1000, tileWorldUnits: 128,
  };
  const camera: RenderCamera = { x: 1, y: 2, height: 3, pitch: 2000, yaw: 0, unitsPerTurn: 16384, zoom: 500, near: 1, far: 5000 };
  const input = new InputController(surface as unknown as HTMLCanvasElement, ui, renderer, services, camera, controls, { x: 3200, y: 3200, plane: 0 });
  const click = (button = 0) => { surface.dispatchEvent(pointer("pointerdown", { button })); surface.dispatchEvent(pointer("pointerup", { button })); };
  try {
    click();
    assert.deepEqual(intents, [{ kind: "walk", destination: { x: 3201, y: 3200, plane: 0 }, running: false }]);
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
