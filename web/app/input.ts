import type { AppServices, RenderCamera, RendererHandle, Tile, UiHandle } from "../shared/contracts.ts";
import type { SourceCameraControls } from "./manifest.ts";
import { sourceUiAdapter } from "./ui-adapter.ts";
import type { UiWorldAdapter } from "./ui-adapter.ts";
import { AppError } from "./errors.ts";

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

  constructor(surface: HTMLCanvasElement, ui: UiHandle, renderer: RendererHandle, services: AppServices,
    camera: RenderCamera, controls: SourceCameraControls | null, focus: Tile,
    renderSize: () => { width: number; height: number } = () => ({ width: surface.width, height: surface.height }),
    uiAdapter: UiWorldAdapter = sourceUiAdapter) {
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
    this.#stopCamera = uiAdapter.cameraRequests(ui, (yaw) => {
      if (Number.isFinite(yaw)) this.#rotate(yaw - this.#camera.yaw, 0);
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
    this.#applyCamera();
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
      || event.composedPath().some(textInput);
  }

  #pick(x: number, y: number) {
    const dimensions = this.#renderSize();
    return this.#renderer.pick(x * dimensions.width / this.#surface.width, y * dimensions.height / this.#surface.height);
  }

  #down = (event: PointerEvent): void => {
    this.#pointer = null;
    const { x, y } = this.#point(event);
    if (this.#blocked(event) || textInput(document.activeElement) || this.#ui.capturesPointer(x, y)
      || (event.button !== 0 && event.button !== 1)) return;
    this.#pointer = { id: event.pointerId, button: event.button, x, y, startX: x, startY: y };
    if (event.button === 1) {
      event.preventDefault();
      if (!this.#controls) {
        this.#pointer = null;
        this.#services.report(new AppError("This explicit source fixture uses its recorded camera; live mouse-camera bindings are not supplied.", { kind: "integration" }));
        return;
      }
      this.#surface.setPointerCapture(event.pointerId);
    }
  };

  #move = (event: PointerEvent): void => {
    const pointer = this.#pointer;
    if (this.#blocked(event)) return;
    const { x, y } = this.#point(event);
    if (!pointer || pointer.id !== event.pointerId || pointer.button !== 1) {
      if (!this.#ui.capturesPointer(x, y) && !textInput(document.activeElement)) {
        this.#uiAdapter.pointer(this.#ui, { kind: "move", x, y, pick: this.#pick(x, y), control: event.ctrlKey });
      }
      return;
    }
    if (!this.#controls) return;
    this.#rotate((x - pointer.x) * this.#controls.yawUnitsPerPixel, (y - pointer.y) * this.#controls.pitchUnitsPerPixel);
    pointer.x = x;
    pointer.y = y;
    event.preventDefault();
  };

  #up = (event: PointerEvent): void => {
    const pointer = this.#pointer;
    this.#pointer = null;
    if (!pointer || pointer.id !== event.pointerId) return;
    if (pointer.button === 1) {
      if (this.#surface.hasPointerCapture(event.pointerId)) this.#surface.releasePointerCapture(event.pointerId);
      return;
    }
    const { x, y } = this.#point(event);
    if (event.button !== 0 || this.#blocked(event) || textInput(document.activeElement)
      || this.#surface.hasPointerCapture(event.pointerId) || this.#ui.capturesPointer(x, y)
      || Math.hypot(x - pointer.startX, y - pointer.startY) > 5) return;
    this.#uiAdapter.pointer(this.#ui, { kind: "primary", x, y, pick: this.#pick(x, y), control: event.ctrlKey });
  };

  #context = (event: MouseEvent): void => {
    const { x, y } = this.#point(event);
    this.#pointer = null;
    if (this.#blocked(event) || this.#ui.capturesPointer(x, y)) return;
    event.preventDefault();
    this.#uiAdapter.pointer(this.#ui, { kind: "context", x, y, pick: this.#pick(x, y), control: event.ctrlKey });
  };

  #wheel = (event: WheelEvent): void => {
    const { x, y } = this.#point(event);
    if (this.#blocked(event) || textInput(document.activeElement) || this.#ui.capturesPointer(x, y)) return;
    event.preventDefault();
    if (!this.#controls) {
      this.#services.report(new AppError("This source composition uses its native viewport zoom; no live scroll-zoom policy was invented.", { kind: "integration" }));
      return;
    }
    if (event.deltaY !== 0) this.#zoom(Math.sign(event.deltaY) * this.#controls.zoomPerWheelStep);
  };

  #keyDown = (event: KeyboardEvent): void => {
    if (!this.#controls || this.#blocked(event) || textInput(document.activeElement) || event.altKey || event.metaKey || event.ctrlKey) return;
    if (["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown"].includes(event.code)) {
      this.#keys.add(event.code);
      event.preventDefault();
    }
  };
  #keyUp = (event: KeyboardEvent): void => { this.#keys.delete(event.code); };
  #cancel = (): void => { this.#pointer = null; this.#keys.clear(); };

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
    const offset = this.#camera.zoom - this.#baseZoom;
    this.#baseZoom = zoom;
    this.#camera.zoom = this.#controls
      ? Math.min(this.#controls.maximumZoom, Math.max(this.#controls.minimumZoom, zoom + offset))
      : zoom;
    this.#applyCamera();
  }

  update(milliseconds: number): void {
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
