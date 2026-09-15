import type { AppServices, RenderCamera, RendererHandle, ScenePick, Tile, UiHandle } from "../shared/contracts.ts";
import type { SourceCameraControls } from "./manifest.ts";
import { AppError } from "./errors.ts";

export function textInput(target: EventTarget | null): boolean {
  if (!target || typeof target !== "object") return false;
  const element = target as Partial<HTMLElement>;
  return ["INPUT", "TEXTAREA", "SELECT", "BUTTON"].includes(element.tagName ?? "")
    || element.isContentEditable === true || element.getAttribute?.("role") === "textbox";
}

export interface WorldContextUi extends UiHandle {
  worldContext?(pick: ScenePick | null, x: number, y: number): void;
}

export class InputController {
  #surface: HTMLCanvasElement;
  #ui: WorldContextUi;
  #renderer: RendererHandle;
  #services: AppServices;
  #camera: RenderCamera;
  #controls: SourceCameraControls;
  #keys = new Set<string>();
  #pointer: { id: number; button: number; x: number; y: number; startX: number; startY: number } | null = null;
  #listeners = new AbortController();
  #focus: Tile;

  constructor(surface: HTMLCanvasElement, ui: UiHandle, renderer: RendererHandle, services: AppServices,
    camera: RenderCamera, controls: SourceCameraControls, focus: Tile) {
    this.#surface = surface;
    this.#ui = ui;
    this.#renderer = renderer;
    this.#services = services;
    this.#camera = { ...camera };
    this.#controls = { ...controls };
    this.#focus = { ...focus };
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
    this.#renderer.camera(this.#camera);
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

  #down = (event: PointerEvent): void => {
    this.#pointer = null;
    const { x, y } = this.#point(event);
    if (this.#blocked(event) || textInput(document.activeElement) || this.#ui.capturesPointer(x, y)
      || (event.button !== 0 && event.button !== 1)) return;
    this.#pointer = { id: event.pointerId, button: event.button, x, y, startX: x, startY: y };
    if (event.button === 1) {
      event.preventDefault();
      this.#surface.setPointerCapture(event.pointerId);
    }
  };

  #move = (event: PointerEvent): void => {
    const pointer = this.#pointer;
    if (!pointer || pointer.id !== event.pointerId || pointer.button !== 1 || this.#blocked(event)) return;
    const { x, y } = this.#point(event);
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
    const pick = this.#renderer.pick(x, y);
    if (!pick) return;
    const world = this.#services.state().world;
    if (!world) return;
    if (pick.kind === "tile") {
      const run = world.player.settings.find((setting) => setting.setting === "run");
      if (!run) {
        this.#services.report(new AppError("The server has not supplied the source run setting.", { kind: "state" }));
        return;
      }
      void this.#services.send({ kind: "walk", destination: { ...pick.tile }, running: run.enabled !== event.ctrlKey }).catch(() => {});
    } else {
      const entity = world.entities.find((entity) => entity.id === pick.id);
      const action = entity?.actions.find((action) => action.allowed);
      if (!entity || !action) {
        this.#services.report(new AppError(entity?.actions[0]?.reason ?? "No authorized interaction is available for this target.", { kind: "state" }));
        return;
      }
      void this.#services.send({
        kind: "interact_with",
        target: entity.kind === "temporary_object" ? { kind: "temporary_object", object: entity.id } : { kind: "spawn", spawn: entity.id },
        action: action.name,
      }).catch(() => {});
    }
  };

  #context = (event: MouseEvent): void => {
    const { x, y } = this.#point(event);
    this.#pointer = null;
    if (this.#blocked(event) || this.#ui.capturesPointer(x, y)) return;
    event.preventDefault();
    this.#ui.worldContext?.(this.#renderer.pick(x, y), x, y);
  };

  #wheel = (event: WheelEvent): void => {
    const { x, y } = this.#point(event);
    if (this.#blocked(event) || textInput(document.activeElement) || this.#ui.capturesPointer(x, y)) return;
    event.preventDefault();
    if (event.deltaY !== 0) this.#zoom(Math.sign(event.deltaY) * this.#controls.zoomPerWheelStep);
  };

  #keyDown = (event: KeyboardEvent): void => {
    if (this.#blocked(event) || textInput(document.activeElement) || event.altKey || event.metaKey || event.ctrlKey) return;
    if (["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown"].includes(event.code)) {
      this.#keys.add(event.code);
      event.preventDefault();
    }
  };
  #keyUp = (event: KeyboardEvent): void => { this.#keys.delete(event.code); };
  #cancel = (): void => { this.#pointer = null; this.#keys.clear(); };

  #rotate(yaw: number, pitch: number): void {
    this.#camera.yaw = ((this.#camera.yaw + yaw) % 16384 + 16384) % 16384;
    this.#camera.pitch = Math.min(this.#controls.maximumPitch, Math.max(this.#controls.minimumPitch, this.#camera.pitch + pitch));
    this.#renderer.camera({ ...this.#camera });
  }
  #zoom(delta: number): void {
    this.#camera.zoom = Math.min(this.#controls.maximumZoom, Math.max(this.#controls.minimumZoom, this.#camera.zoom + delta));
    this.#renderer.camera({ ...this.#camera });
  }

  update(milliseconds: number): void {
    if (this.#services.state().phase !== "world" || textInput(document.activeElement)) { this.#keys.clear(); return; }
    const seconds = Math.min(100, Math.max(0, milliseconds)) / 1000;
    const yaw = (Number(this.#keys.has("ArrowRight")) - Number(this.#keys.has("ArrowLeft"))) * seconds * this.#controls.keyboardYawUnitsPerSecond;
    const pitch = (Number(this.#keys.has("ArrowDown")) - Number(this.#keys.has("ArrowUp"))) * seconds * this.#controls.keyboardPitchUnitsPerSecond;
    if (yaw !== 0 || pitch !== 0) this.#rotate(yaw, pitch);
    const tile = this.#services.state().world?.player.tile;
    if (tile && (tile.x !== this.#focus.x || tile.y !== this.#focus.y)) {
      this.#camera.x += (tile.x - this.#focus.x) * this.#controls.tileWorldUnits;
      this.#camera.y += (tile.y - this.#focus.y) * this.#controls.tileWorldUnits;
      this.#focus = { ...tile };
      this.#renderer.camera({ ...this.#camera });
    }
  }
  dispose(): void {
    this.#listeners.abort();
    this.#cancel();
  }
}
