import type { AppServices, RenderCamera, RendererHandle, Tile, UiHandle } from "../shared/contracts.ts";
import type { SourceCameraControls } from "./manifest.ts";
import { sourceUiAdapter } from "./ui-adapter.ts";
import type { UiWorldAdapter } from "./ui-adapter.ts";
import { AppError, appError } from "./errors.ts";
import { cameraNanoseconds, cameraWheelRotation } from "./camera.ts";
import type { NativeCameraInputSink } from "./camera.ts";

export function textInput(target: EventTarget | null): boolean {
  if (!target || typeof target !== "object") return false;
  const element = target as Partial<HTMLElement>;
  return ["INPUT", "TEXTAREA", "SELECT", "BUTTON"].includes(element.tagName ?? "")
    || element.isContentEditable === true || element.getAttribute?.("role") === "textbox";
}

export class InputController {
  #surface: HTMLCanvasElement;
  #ui: UiHandle;
  #renderer: RendererHandle;
  #services: AppServices;
  #camera: RenderCamera;
  #controls: SourceCameraControls | null;
  #baseZoom: number;
  #keys = new Set<string>();
  #pointer: { id: number; button: number; x: number; y: number; startX: number; startY: number } | null = null;
  #listeners = new AbortController();
  #focus: Tile;
  #uiAdapter: UiWorldAdapter;
  #stopCamera: () => void;
  #renderSize: () => { width: number; height: number };
  #native: NativeCameraInputSink | null;
  #nativeMouse: [number, number] = [0, 0];

  constructor(surface: HTMLCanvasElement, ui: UiHandle, renderer: RendererHandle, services: AppServices,
    camera: RenderCamera, controls: SourceCameraControls | null, focus: Tile,
    renderSize: () => { width: number; height: number } = () => ({ width: surface.width, height: surface.height }),
    uiAdapter: UiWorldAdapter = sourceUiAdapter, native: NativeCameraInputSink | null = null) {
    this.#surface = surface;
    this.#ui = ui;
    this.#renderer = renderer;
    this.#services = services;
    this.#camera = { ...camera };
    this.#controls = controls ? { ...controls } : null;
    this.#baseZoom = camera.zoom;
    this.#focus = { ...focus };
    this.#renderSize = renderSize;
    this.#uiAdapter = uiAdapter;
    this.#native = native;
    const knownMouse = native?.mouse?.();
    if (knownMouse) this.#nativeMouse = [...knownMouse];
    this.#stopCamera = uiAdapter.cameraRequests(ui, (yaw) => {
      if (this.#native) {
        try { this.#native.faceYaw(yaw); }
        catch (error) { this.#services.report(appError(error, "Native compass input failed.")); }
      } else if (Number.isFinite(yaw)) this.#rotate(yaw - this.#camera.yaw, 0);
    });
    const signal = this.#listeners.signal;
    surface.addEventListener("pointerdown", this.#down, { signal });
    surface.addEventListener("pointermove", this.#move, { signal });
    surface.addEventListener("pointerup", this.#up, { signal });
    surface.addEventListener("pointercancel", this.#cancel, { signal });
    surface.addEventListener("lostpointercapture", this.#cancel, { signal });
    surface.addEventListener("wheel", this.#wheel, { signal, passive: false });
    surface.addEventListener("contextmenu", this.#context, { signal });
    document.addEventListener("keydown", this.#keyDown, { signal });
    document.addEventListener("keyup", this.#keyUp, { signal });
    window.addEventListener("blur", this.#cancel, { signal });
    document.addEventListener("visibilitychange", this.#visibility, { signal });
    if (this.#native) {
      this.publishNativeCamera(camera);
      if (knownMouse) this.#sendNative();
    }
    else this.#applyCamera();
  }

  #point(event: MouseEvent): { x: number; y: number } {
    const bounds = this.#surface.getBoundingClientRect();
    return {
      x: (event.clientX - bounds.left) * this.#surface.width / bounds.width,
      y: (event.clientY - bounds.top) * this.#surface.height / bounds.height,
    };
  }

  #blocked(event: Event): boolean {
    return event.defaultPrevented || this.#services.state().phase !== "world"
      || (this.#native !== null && !this.#native.ready())
      || event.composedPath().some(textInput);
  }

  #pick(x: number, y: number) {
    const dimensions = this.#renderSize();
    return this.#renderer.pick(x * dimensions.width / this.#surface.width, y * dimensions.height / this.#surface.height);
  }

  #down = (event: PointerEvent): void => {
    if (this.#pointer !== null) return;
    this.#pointer = null;
    const { x, y } = this.#point(event);
    if (this.#blocked(event) || textInput(document.activeElement) || this.#ui.capturesPointer(x, y)
      || (event.button !== 0 && event.button !== 1)) return;
    this.#pointer = { id: event.pointerId, button: event.button, x, y, startX: x, startY: y };
    this.#nativeMouse = [Math.trunc(x), Math.trunc(y)];
    if (event.button === 1) {
      event.preventDefault();
      if (!this.#controls && !this.#native) {
        this.#pointer = null;
        this.#services.report(new AppError("This explicit source fixture uses its recorded camera; live mouse-camera bindings are not supplied.", { kind: "integration" }));
        return;
      }
      this.#surface.setPointerCapture(event.pointerId);
    }
    this.#sendNative();
  };

  #move = (event: PointerEvent): void => {
    const pointer = this.#pointer;
    if (this.#blocked(event) || textInput(document.activeElement)) {
      if (this.#native) this.#cancel();
      return;
    }
    const { x, y } = this.#point(event);
    this.#nativeMouse = [Math.trunc(x), Math.trunc(y)];
    this.#sendNative();
    if (!pointer || pointer.id !== event.pointerId || pointer.button !== 1) {
      if (!this.#ui.capturesPointer(x, y) && !textInput(document.activeElement)) {
        this.#uiAdapter.pointer(this.#ui, { kind: "move", x, y, pick: this.#pick(x, y), control: event.ctrlKey });
      }
      return;
    }
    if (this.#native) { event.preventDefault(); return; }
    if (!this.#controls) return;
    this.#rotate((x - pointer.x) * this.#controls.yawUnitsPerPixel, (y - pointer.y) * this.#controls.pitchUnitsPerPixel);
    pointer.x = x;
    pointer.y = y;
    event.preventDefault();
  };

  #up = (event: PointerEvent): void => {
    const pointer = this.#pointer;
    if (!pointer || pointer.id !== event.pointerId) return;
    this.#pointer = null;
    const { x, y } = this.#point(event);
    this.#nativeMouse = [Math.trunc(x), Math.trunc(y)];
    this.#sendNative();
    if (pointer.button === 1) {
      if (this.#surface.hasPointerCapture(event.pointerId)) this.#surface.releasePointerCapture(event.pointerId);
      return;
    }
    if (event.button !== 0 || this.#blocked(event) || textInput(document.activeElement)
      || this.#surface.hasPointerCapture(event.pointerId) || this.#ui.capturesPointer(x, y)
      || Math.hypot(x - pointer.startX, y - pointer.startY) > 5) return;
    this.#uiAdapter.pointer(this.#ui, { kind: "primary", x, y, pick: this.#pick(x, y), control: event.ctrlKey });
  };

  #context = (event: MouseEvent): void => {
    const { x, y } = this.#point(event);
    this.#cancel();
    if (this.#blocked(event) || this.#ui.capturesPointer(x, y)) return;
    event.preventDefault();
    this.#uiAdapter.pointer(this.#ui, { kind: "context", x, y, pick: this.#pick(x, y), control: event.ctrlKey });
  };

  #wheel = (event: WheelEvent): void => {
    const { x, y } = this.#point(event);
    if (this.#blocked(event) || textInput(document.activeElement) || this.#ui.capturesPointer(x, y)) return;
    event.preventDefault();
    if (this.#native) {
      try {
        this.#nativeMouse = [Math.trunc(x), Math.trunc(y)];
        this.#sendNative(cameraWheelRotation(event.deltaY, event.deltaMode));
      } catch (error) { this.#services.report(appError(error, "Native wheel input failed.")); }
      return;
    }
    if (!this.#controls) {
      this.#services.report(new AppError("This source composition uses its native viewport zoom; no live scroll-zoom policy was invented.", { kind: "integration" }));
      return;
    }
    if (event.deltaY !== 0) this.#zoom(Math.sign(event.deltaY) * this.#controls.zoomPerWheelStep);
  };

  #keyDown = (event: KeyboardEvent): void => {
    if ((!this.#controls && !this.#native) || this.#blocked(event) || textInput(document.activeElement) || event.altKey || event.metaKey || event.ctrlKey) return;
    if (["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown"].includes(event.code)) {
      const held = this.#keys.has(event.code);
      this.#keys.add(event.code);
      event.preventDefault();
      if (!held) this.#sendNative();
    }
  };
  #keyUp = (event: KeyboardEvent): void => { if (this.#keys.delete(event.code)) this.#sendNative(); };
  #visibility = (): void => { if (document.hidden) this.#cancel(); };
  #cancel = (): void => {
    const pointer = this.#pointer;
    const owned = pointer !== null || this.#keys.size > 0;
    this.#pointer = null;
    this.#keys.clear();
    if (pointer && this.#surface.hasPointerCapture(pointer.id)) this.#surface.releasePointerCapture(pointer.id);
    if (owned) this.#sendNative();
  };

  #sendNative(rotation = 0): void {
    if (!this.#native?.ready()) return;
    try {
      this.#native.input(cameraNanoseconds(performance.now()), {
        arrows: { left: this.#keys.has("ArrowLeft"), right: this.#keys.has("ArrowRight"),
          up: this.#keys.has("ArrowUp"), down: this.#keys.has("ArrowDown") },
        mouse: [...this.#nativeMouse],
        button: this.#pointer?.button === 1 ? "Middle" : this.#pointer?.button === 0 ? "Primary" : "Released",
        wheel: rotation === 0 ? null : { rotation, route: "Camera" },
      });
    } catch (error) { this.#services.report(appError(error, "Native camera input failed.")); }
  }

  publishNativeCamera(camera: RenderCamera): void {
    this.#camera = { ...camera };
    this.#uiAdapter.camera(this.#ui, { ...camera });
  }

  #rotate(yaw: number, pitch: number): void {
    this.#camera.yaw = ((this.#camera.yaw + yaw) % 16384 + 16384) % 16384;
    if (this.#controls) this.#camera.pitch = Math.min(this.#controls.maximumPitch, Math.max(this.#controls.minimumPitch, this.#camera.pitch + pitch));
    this.#applyCamera();
  }
  #zoom(delta: number): void {
    if (!this.#controls) return;
    this.#camera.zoom = Math.min(this.#controls.maximumZoom, Math.max(this.#controls.minimumZoom, this.#camera.zoom + delta));
    this.#applyCamera();
  }

  #applyCamera(): void {
    this.#renderer.camera({ ...this.#camera });
    this.#uiAdapter.camera(this.#ui, { ...this.#camera });
  }

  resizeZoom(zoom: number): void {
    if (this.#native) throw new AppError("Native camera resizing must use Rust viewport/FOV, not the full-HUD fixture zoom.", { kind: "camera_unavailable" });
    const offset = this.#camera.zoom - this.#baseZoom;
    this.#baseZoom = zoom;
    this.#camera.zoom = this.#controls
      ? Math.min(this.#controls.maximumZoom, Math.max(this.#controls.minimumZoom, zoom + offset))
      : zoom;
    this.#applyCamera();
  }

  update(milliseconds: number): void {
    if (this.#native) {
      if (this.#services.state().phase !== "world" || textInput(document.activeElement)) this.#cancel();
      return;
    }
    if (this.#services.state().phase !== "world" || textInput(document.activeElement)) { this.#keys.clear(); return; }
    if (!this.#controls) return;
    const seconds = Math.min(100, Math.max(0, milliseconds)) / 1000;
    const yaw = (Number(this.#keys.has("ArrowRight")) - Number(this.#keys.has("ArrowLeft"))) * seconds * this.#controls.keyboardYawUnitsPerSecond;
    const pitch = (Number(this.#keys.has("ArrowDown")) - Number(this.#keys.has("ArrowUp"))) * seconds * this.#controls.keyboardPitchUnitsPerSecond;
    if (yaw !== 0 || pitch !== 0) this.#rotate(yaw, pitch);
    const tile = this.#services.state().world?.player.tile;
    if (tile && (tile.x !== this.#focus.x || tile.y !== this.#focus.y)) {
      this.#camera.x += (tile.x - this.#focus.x) * this.#controls.tileWorldUnits;
      this.#camera.y += (tile.y - this.#focus.y) * this.#controls.tileWorldUnits;
      this.#focus = { ...tile };
      this.#applyCamera();
    }
  }
  dispose(): void {
    this.#listeners.abort();
    this.#stopCamera();
    this.#cancel();
  }
}
