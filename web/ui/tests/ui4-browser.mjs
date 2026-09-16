import assert from "node:assert/strict";
import { mkdir, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { browserHost, launchBrowser, results } from "./browser-host.mjs";

const host = await browserHost(), browser = await launchBrowser(), cases = [], errors = [];
const page = await browser.newPage({ viewport: { width: 1920, height: 1080 }, deviceScaleFactor: 1 });
page.on("pageerror", error => errors.push(error.message));
const frame = () => page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
const click = async id => { await page.locator(`[data-ui-control="${id}"]`).click(); await frame(); };
const reset = () => page.evaluate(() => { window.component.services.intents.length = 0; });
const intents = () => page.evaluate(() => window.component.services.intents);
const patch = async change => { await page.evaluate(change); await frame(); };
const mount = async () => {
  await page.evaluate(async () => { const fixture = await import("/web/ui/tests/component-fixture.ts"); await fixture.mount("world"); window.component.services.enableUi(); });
  await frame(); await reset();
};
const projectedMinimap = async () => {
  await mount();
  await page.evaluate(async () => {
    const { bindUiMinimapProjection, setUiMapIconSprites } = await import("/web/ui/index.ts");
    const w = window.component.services.state().world, f = window.componentMinimap(w, 2);
    const pixels = new ImageData(3, 3);
    for (let i = 0; i < pixels.data.length; i += 4) pixels.data.set([217, 23, 71, 255], i);
    const sprite = { element: 777, width: 3, height: 3, maxWidth: 15, maxHeight: 17,
      offsetX: 5, offsetY: 6, category: -1, pixels };
    window.suppliedIcon = { element: 777, tileX: w.player.tile.x + 4, tileY: w.player.tile.y + 3,
      x: 25, y: 34, drawX: 30, drawY: 40, dx: 16, dy: 12, clipped: false };
    window.projectionReady = true; window.projectionCalls = 0;
    setUiMapIconSprites(window.component.ui, new Map([[777, sprite]]));
    pixels.data.fill(0);
    bindUiMinimapProjection(window.component.ui, (_width, _height, scale) => {
      window.projectionCalls++;
      if (!window.projectionReady) throw new Error("The fixture renderer has no current authoritative scene/player.");
      window.lastIconProjection = { minimapAngle: 0, scale, missingSprites: 0, icons: [{ ...window.suppliedIcon }] };
      return window.lastIconProjection;
    });
    f.icons = [{ x: w.player.tile.x + 4, y: w.player.tile.y + 3, plane: w.player.tile.plane, element: 777 }];
    window.setUiMinimap(window.component.ui, f);
  });
  await frame();
};
const capture = async name => {
  await page.mouse.move(600, 100); await page.waitForLoadState("networkidle"); await frame();
  await page.screenshot({ path: resolve(results, "ui4-components", name + ".png") });
};
async function check(name, run) {
  try { await run(); cases.push({ name, passed: true }); }
  catch (error) {
    cases.push({ name, passed: false, error: error.message,
      status: await page.getByRole("status").innerText() });
    throw error;
  }
}

try {
  await page.goto(host.url); await mkdir(resolve(results, "ui4-components"), { recursive: true });
  await check("activeTab is authoritative, independent from a contextual interface, and is not selected on rejection or an empty acknowledgement", async () => {
    await mount();
    await page.evaluate(() => { window.component.services.automaticTabReplies = false; });
    await click("tab-11");
    assert.equal(await page.locator('[data-ui-control="tab-3"]').getAttribute("aria-pressed"), "true");
    assert.equal(await page.locator('[data-ui-control="tab-11"]').getAttribute("aria-pressed"), "false");
    await patch(() => {
      const s = window.component.services, world = structuredClone(s.state().world);
      world.ui.activeTab = "interface.settings"; world.ui.activeInterface = "interface.inventory";
      s.patchWorld(world);
    });
    assert.equal(await page.locator('[data-ui-control="tab-11"]').getAttribute("aria-pressed"), "true");
    await page.evaluate(() => { window.component.services.rejection = { message: "Actual tab rejection", errorId: "ui4.tab.reject" }; });
    await click("tab-0");
    assert.match(await page.getByRole("status").innerText(), /ui4.tab.reject/);
    await click("notice-close");
    assert.equal(await page.locator('[data-ui-control="tab-11"]').getAttribute("aria-pressed"), "true");
    await patch(() => {
      const s = window.component.services, world = structuredClone(s.state().world);
      world.ui.activeTab = null; world.ui.activeInterface = null; s.patchWorld(world);
    });
    assert.equal(await page.locator('[data-ui-control^="tab-"][aria-pressed="true"]').count(), 0);
    assert.equal(await page.locator('[data-ui-control="inventory-0"]').count(), 0);
  });

  await check("bank placeholders echo the displayed bank revision, preserve banker context and reject stale held quantities without rebinding", async () => {
    await mount();
    await patch(() => {
      const s = window.component.services, w = structuredClone(s.state().world), item = w.player.inventory[0].item;
      w.bank = { banker: "banker.actual", capacity: 400, slots: [{ index: 0, item }], allowNotes: true };
      w.ui.bank = { revision: "9007199254740993", capacity: 400, selectedTab: 0, insertMode: false, placeholders: false, amount: 1, noted: false,
        entries: [{ id: "entry.stable", slot: 0, tab: 0, item: item.id, value: item, placeholder: false }],
        tabs: [{ tab: 0, firstEntry: "entry.stable", entries: 1 }], depositEquipment: { allowed: true, code: null, reason: null }, unavailableContainers: [] };
      s.patchWorld(w);
    });
    await page.locator('[data-ui-control="bank-entry-entry.stable"]').click({ button: "right" }); await frame();
    await page.getByRole("button", { name: "Placeholder Bronze pickaxe", exact: true }).click(); await frame();
    assert.deepEqual((await intents()).at(-1), { kind: "bank_placeholder", entry_id: "entry.stable", expected_bank_revision: "9007199254740993" });
    assert.equal(await page.evaluate(() => window.component.services.state().world.ui.bank.entries[0].placeholder), false);
    await click("inventory-0");
    assert.equal((await intents()).at(-1).banker, "banker.actual");
    await page.locator('[data-ui-control="bank-entry-entry.stable"]').click({ button: "right" }); await frame();
    await page.getByRole("button", { name: "Withdraw-X Bronze pickaxe", exact: true }).click(); await frame();
    await page.getByRole("textbox", { name: "Enter amount:", exact: true }).fill("7");
    await patch(() => {
      const s = window.component.services, w = structuredClone(s.state().world);
      w.ui.bank.revision = "9007199254740994"; s.patchWorld(w);
    }); await reset();
    await page.getByRole("textbox", { name: "Enter amount:", exact: true }).press("Enter"); await frame();
    assert.equal((await intents()).length, 0);
    assert.match(await page.getByRole("status").innerText(), /ui.bank.revision.stale/);
    await click("notice-close");
    assert.equal(await page.getByRole("textbox", { name: "Enter amount:", exact: true }).inputValue(), "7");
    await page.locator("canvas").press("Escape"); await frame();
  });

  await check("original book controls retain document identity and authoritative page state across acknowledgement, rejection and cancellation", async () => {
    await mount();
    await patch(() => {
      const s = window.component.services, w = structuredClone(s.state().world);
      w.ui.activeInterface = "interface.read_book";
      w.ui.document = { id: "book.actual", interface: "interface.read_book", title: "Authoritative component book",
        pages: ["<col=7f0000>First supplied page</col>", "Second supplied page", "Last supplied page"], page: 0, nativeMap: false, mapAsset: null };
      s.patchWorld(w);
    });
    await click("document-next");
    assert.deepEqual((await intents()).at(-1), { kind: "ui_document_page", document_id: "book.actual", page: 1 });
    assert.equal(await page.evaluate(() => window.component.services.state().world.ui.document.page), 0);
    assert.equal(await page.locator('[data-ui-control="document-previous"]').count(), 0);
    await page.evaluate(() => { window.component.services.rejection = { message: "Actual document page rejection", errorId: "ui4.document.reject" }; });
    await click("document-next");
    assert.match(await page.getByRole("status").innerText(), /ui4.document.reject/);
    await click("notice-close");
    await patch(() => {
      const s = window.component.services, w = structuredClone(s.state().world); w.ui.document.page = 1; s.patchWorld(w);
    });
    await capture("native-book-authoritative-page");
    await page.locator("canvas").press("ArrowLeft"); await frame();
    assert.deepEqual((await intents()).at(-1), { kind: "ui_document_page", document_id: "book.actual", page: 0 });
    await page.locator("canvas").press("Escape"); await frame();
    assert.deepEqual((await intents()).at(-1), { kind: "ui_dismiss", presentation_id: "book.actual" });
    assert.equal(await page.locator('[data-ui-control="document-close"]').count(), 1);
    await patch(() => {
      const s = window.component.services, w = structuredClone(s.state().world);
      w.ui.document.page = 0; w.ui.document.pages = ["Long source text ".repeat(500)]; s.patchWorld(w);
    });
    await page.locator('[data-ui-control="document-next"]').click({ button: "right" }); await frame();
    await patch(() => {
      const s = window.component.services, w = structuredClone(s.state().world);
      w.ui.document.id = "book.replaced"; w.ui.document.pages = ["Replacement text"]; s.patchWorld(w);
    }); await reset();
    await page.getByRole("button", { name: "Next page", exact: true }).click(); await frame();
    assert.equal((await intents()).length, 0);
    assert.match(await page.getByRole("status").innerText(), /ui.presentation.stale/);
  });

  await check("newcomer-map native models and tutor controls do not invent pages, while supplied map artwork uses native model-only bounds", async () => {
    await mount();
    await patch(() => {
      const s = window.component.services, w = structuredClone(s.state().world);
      w.ui.activeInterface = "interface.newcomer_map";
      w.ui.document = { id: "map.actual", interface: "interface.newcomer_map", title: "Newcomer map", pages: [], page: 0, nativeMap: true, mapAsset: null };
      s.patchWorld(w);
    });
    assert.equal(await page.locator('[data-ui-control="document-next"]').count(), 0);
    await reset(); await click("document-tutors");
    assert.equal((await intents()).length, 0);
    assert.equal(await page.getByRole("button", { name: "Hide Tutors", exact: true }).count(), 1);
    await capture("native-newcomer-map-tutors");
    await page.evaluate(async () => {
      const { decodeUiCatalogue } = await import("/web/ui/assets.ts");
      const c = decodeUiCatalogue(await (await fetch("/assets/ui/manifest.json")).json());
      const source = Object.values(c.staticModels).find(value => value.widget.model === 3024);
      const s = window.component.services, w = structuredClone(s.state().world);
      w.ui.document.mapAsset = source.asset; s.patchWorld(w);
    }); await frame();
    await page.waitForLoadState("networkidle"); await frame();
    assert.doesNotMatch(await page.getByRole("status").innerText(), /ui.document.asset.size/);
    await click("document-close");
    assert.deepEqual((await intents()).at(-1), { kind: "ui_dismiss", presentation_id: "map.actual" });
  });

  await check("canonical source233 level-up text uses supplied values and continuation without client XP or fabricated close events", async () => {
    await mount();
    await patch(() => {
      const s = window.component.services, w = structuredClone(s.state().world);
      w.ui.reward = { id: "level.actual", kind: "level_up", interface: "interface.level_up", title: "Actual level-up title",
        lines: ["Actual committed level-up text."], items: [], xp: [{ skill: "skill.mining", amountTenths: "350" }],
        questPoints: 0, quest: null, skill: "skill.mining", level: 2, continuation: { kind: "ui_dismiss", presentation_id: "level.actual" } };
      s.patchWorld(w);
    });
    assert.equal(await page.locator('[data-ui-control="level-up-continue"]').count(), 1);
    assert.doesNotMatch(await page.getByRole("status").innerText(), /ui.source.reward.layout/);
    const before = await page.evaluate(() => window.component.services.state().world.player.skills);
    await capture("canonical-levelup-source233");
    await click("level-up-continue");
    assert.deepEqual((await intents()).at(-1), { kind: "ui_dismiss", presentation_id: "level.actual" });
    assert.deepEqual(await page.evaluate(() => window.component.services.state().world.player.skills), before);
    assert.equal(await page.evaluate(() => window.component.services.state().world.ui.reward.id), "level.actual");
  });

  await check("renderer minimap pixels are copied once per revision and stale/reused/malformed surfaces explicitly fail", async () => {
    await mount();
    const bounds = await page.locator('[data-ui-control="minimap"]').boundingBox();
    const sample = { x: Math.floor(bounds.x + bounds.width / 2 + 20), y: Math.floor(bounds.y + bounds.height / 2) };
    const colour = () => page.evaluate(({ x, y }) => Array.from(document.querySelector("canvas").getContext("2d").getImageData(x, y, 1, 1).data), sample);
    await page.evaluate(() => {
      const f = window.componentMinimap(window.component.services.state().world, 2);
      for (let pixel = 0; pixel < f.pixels.data.length; pixel += 4) f.pixels.data.set([19, 73, 111, 255], pixel);
      f.mask.fill(1); f.notes = ["Actual supplied incomplete-edge note"]; f.stats.unresolved = 7;
      window.setUiMinimap(window.component.ui, f);
      f.pixels.data.fill(0); f.notes.push("Producer mutation must not leak");
    }); await frame();
    assert.deepEqual(await colour(), [19, 73, 111, 255]);
    assert.deepEqual(await page.evaluate(() => window.getUiMinimapStatus(window.component.ui).notes), ["Actual supplied incomplete-edge note"]);
    assert.equal(await page.evaluate(() => window.getUiMinimapStatus(window.component.ui).complete), false);
    assert.equal(await page.evaluate(() => window.getUiMinimapStatus(window.component.ui).stats.unresolved), 7);
    const failure = async damage => page.evaluate(damage);
    assert.equal(await failure(() => {
      try { window.setUiMinimap(window.component.ui, window.componentMinimap(window.component.services.state().world, 1)); }
      catch (error) { return error.errorId; }
    }), "ui.minimap.stale");
    await click("notice-close");
    assert.equal(await failure(() => {
      try { window.setUiMinimap(window.component.ui, window.componentMinimap(window.component.services.state().world, 2)); }
      catch (error) { return error.errorId; }
    }), "ui.minimap.revision_reused");
    await click("notice-close");
    assert.equal(await failure(() => {
      const f = window.componentMinimap(window.component.services.state().world, 3); f.mask = new Uint8Array(1);
      try { window.setUiMinimap(window.component.ui, f); } catch (error) { return error.errorId; }
    }), "ui.minimap.invalid");
    await click("notice-close");
    assert.equal(await page.evaluate(() => window.getUiMinimapStatus(window.component.ui).revision), 2);
    await capture("revisioned-minimap-component-surface");
  });

  await check("minimap owner/instance changes clear stale data while observer null/absence and exact timestamps survive UI updates", async () => {
    await mount();
    await patch(() => {
      const s = window.component.services, w = structuredClone(s.state().world);
      w.player.instance = "instance.next";
      w.player.running = true; w.player.movementTick = "18446744073709551610";
      w.player.action = { version: 1, id: "action.stable", activity: "production", actionId: null, target: null,
        recipeId: "recipe.actual", styleId: null, spellId: null, animation: "animation.actual",
        startedAtTick: "18446744073709551600", cycleStartedAtTick: "18446744073709551605",
        nextActionTick: null, observedAtTick: "18446744073709551610" };
      w.entities[0].action = null;
      w.dynamicObjects = [{ id: "dynamic.actual", objectId: "object.actual", sourceId: 1535, tile: w.player.tile, instance: "instance.next", state: "open", doorOpen: true, quarterTurns: 1 }];
      s.patchWorld(w);
    });
    assert.equal(await page.evaluate(() => window.getUiMinimapStatus(window.component.ui)), null);
    assert.equal(await page.locator('[data-ui-control="minimap"]').isDisabled(), true);
    const before = await page.evaluate(() => {
      const w = window.component.services.state().world;
      return { running: w.player.running, movement: w.player.movementTick, action: w.player.action, other: w.entities[0].action, dynamic: w.dynamicObjects };
    });
    await page.evaluate(() => window.setUiMinimap(window.component.ui, window.componentMinimap(window.component.services.state().world, 1)));
    await page.evaluate(() => window.component.ui.resize(1024, 768)); await frame();
    await page.evaluate(() => window.component.ui.resize(1920, 1080)); await frame();
    assert.deepEqual(await page.evaluate(() => {
      const w = window.component.services.state().world;
      return { running: w.player.running, movement: w.player.movementTick, action: w.player.action, other: w.entities[0].action, dynamic: w.dynamicObjects };
    }), before);
    assert.equal(await page.evaluate(() => Object.hasOwn(window.component.services.state().world.entities[0], "running")), false);
  });

  await check("map-element identities remain exact and missing original icons are visible in diagnostics instead of generic substitutes", async () => {
    await mount();
    await page.evaluate(async () => {
      const { decodeUiCatalogue } = await import("/web/ui/assets.ts");
      const { bindUiMinimapProjection, setUiMapIconSprites } = await import("/web/ui/index.ts");
      const c = decodeUiCatalogue(await (await fetch("/assets/ui/manifest.json")).json());
      const element = Object.values(c.mapElements).find(value => value.sprite >= 0 && c.sprites[value.sprite]);
      const sprite = c.sprites[element.sprite], shape = sprite.frames[0];
      const image = new Image(); image.src = "/assets/" + sprite.asset; await image.decode();
      const source = document.createElement("canvas"); source.width = image.naturalWidth; source.height = image.naturalHeight;
      const context = source.getContext("2d"); context.drawImage(image, 0, 0);
      const pixels = context.getImageData(shape.x, shape.y, shape.width, shape.height).data;
      let sample = 0;
      while (sample < shape.width * shape.height && pixels[sample * 4 + 3] !== 255) sample++;
      if (sample >= shape.width * shape.height) throw new Error("Original map icon has no opaque proof pixel.");
      const bounds = document.querySelector('[data-ui-control="minimap"]').getBoundingClientRect();
      window.expectedIconPixel = {
        x: Math.floor(bounds.x + bounds.width / 2) + 16 - Math.trunc(shape.canvasWidth / 2) + shape.offsetX + sample % shape.width,
        y: Math.floor(bounds.y + bounds.height / 2) - 12 - Math.trunc(shape.canvasHeight / 2) + shape.offsetY + Math.floor(sample / shape.width),
        colour: Array.from(pixels.slice(sample * 4, sample * 4 + 4)),
      };
      const w = window.component.services.state().world, f = window.componentMinimap(w, 2);
      setUiMapIconSprites(window.component.ui, new Map([[element.sourceId, {
        element: element.sourceId, width: shape.width, height: shape.height,
        maxWidth: shape.canvasWidth, maxHeight: shape.canvasHeight,
        offsetX: shape.offsetX, offsetY: shape.offsetY, category: -1,
        pixels: new ImageData(pixels.slice(), shape.width, shape.height),
      }]]));
      bindUiMinimapProjection(window.component.ui, (width, height, scale) => {
        const x = (width >> 1) + 16 - Math.trunc(shape.canvasWidth / 2);
        const y = (height >> 1) - 12 - Math.trunc(shape.canvasHeight / 2);
        return { minimapAngle: 0, scale, missingSprites: 1, icons: [{
          element: element.sourceId, tileX: w.player.tile.x + 4, tileY: w.player.tile.y + 3,
          x, y, drawX: x + shape.offsetX, drawY: y + shape.offsetY, dx: 16, dy: 12, clipped: false,
        }] };
      });
      f.icons = [{ x: w.player.tile.x + 4, y: w.player.tile.y + 3, plane: w.player.tile.plane, element: element.sourceId },
        { x: w.player.tile.x - 3, y: w.player.tile.y - 4, plane: w.player.tile.plane, element: 999999 }];
      window.setUiMinimap(window.component.ui, f); window.expectedMapElement = element.sourceId;
    }); await frame();
    assert.equal(await page.evaluate(() => window.getUiMinimapStatus(window.component.ui).icons[0].element), await page.evaluate(() => window.expectedMapElement));
    assert.deepEqual(await page.evaluate(() => window.getUiMinimapStatus(window.component.ui).missingElements), [999999]);
    assert.match(await page.locator('[data-ui-control="minimap"]').getAttribute("aria-description"), /999999/);
    assert.deepEqual(await page.evaluate(() => {
      const sample = window.expectedIconPixel;
      return Array.from(document.querySelector("canvas").getContext("2d").getImageData(sample.x, sample.y, 1, 1).data);
    }), await page.evaluate(() => window.expectedIconPixel.colour));
    await capture("original-map-element-component-icons");
  });
  await check("renderer-owned trim positions are used directly, copied sprites stay immutable and masked blits stay inside the source aperture", async () => {
    await projectedMinimap();
    const bounds = await page.locator('[data-ui-control="minimap"]').boundingBox();
    const pixel = (x, y) => page.evaluate(({ x, y }) => Array.from(document.querySelector("canvas")
      .getContext("2d").getImageData(x, y, 1, 1).data), { x: Math.floor(bounds.x + x), y: Math.floor(bounds.y + y) });
    const corner = await pixel(1, 1);
    assert.deepEqual(await pixel(30, 40), [217, 23, 71, 255]);
    await page.evaluate(() => {
      window.suppliedIcon.clipped = true;
      window.component.ui.resize(1920, 1080);
    }); await frame();
    assert.deepEqual(await pixel(30, 40), [217, 23, 71, 255], "No second sprite trim offset is applied.");
    await page.evaluate(() => {
      window.suppliedIcon.drawX = 0; window.suppliedIcon.drawY = 0;
      window.component.ui.resize(1920, 1080);
    }); await frame();
    assert.deepEqual(await pixel(1, 1), corner, "The original aperture clips even a supplied in-widget corner placement.");
    assert.equal(await page.evaluate(async () => {
      const { setUiMapIconSprites } = await import("/web/ui/index.ts");
      try { setUiMapIconSprites(window.component.ui, new Map([[777, {
        element: 777, width: 3, height: 3, maxWidth: 15, maxHeight: 17,
        offsetX: 5, offsetY: 6, category: -1, pixels: new ImageData(2, 2),
      }]])); } catch (error) { return error.errorId; }
    }), "ui.minimap.sprite");
  });
  await check("reconnect retains the matching copied native projection without querying an unavailable renderer or leaking it across scopes", async () => {
    await projectedMinimap();
    const pixel = () => page.evaluate(() => {
      const bounds = document.querySelector('[data-ui-control="minimap"]').getBoundingClientRect();
      return Array.from(document.querySelector("canvas").getContext("2d")
        .getImageData(Math.floor(bounds.x + 30), Math.floor(bounds.y + 40), 1, 1).data);
    });
    const calls = await page.evaluate(() => window.projectionCalls);
    assert.deepEqual(await pixel(), [217, 23, 71, 255]);
    await patch(() => {
      window.projectionReady = false;
      window.lastIconProjection.icons[0].drawX = 0;
      const s = window.component.services; s.publish({ ...s.state(), phase: "reconnecting" });
    });
    assert.equal(await page.evaluate(() => window.projectionCalls), calls);
    assert.deepEqual(await pixel(), [217, 23, 71, 255]);
    assert.match(await page.locator('[data-ui-control="minimap"]').getAttribute("aria-description"), /paused/);
    await patch(() => {
      window.projectionReady = true;
      const s = window.component.services; s.publish({ ...s.state(), phase: "world" });
    });
    assert((await page.evaluate(() => window.projectionCalls)) > calls);
    assert.deepEqual(await pixel(), [217, 23, 71, 255]);
    assert.doesNotMatch(await page.locator('[data-ui-control="minimap"]').getAttribute("aria-description"), /paused/);
    await patch(() => {
      window.projectionReady = false;
      const s = window.component.services, world = structuredClone(s.state().world);
      world.player.instance = "instance.reconnect.other";
      s.publish({ ...s.state(), phase: "reconnecting", world });
    });
    assert.equal(await page.evaluate(() => window.getUiMinimapStatus(window.component.ui)), null);
    assert.notDeepEqual(await pixel(), [217, 23, 71, 255]);
    assert.equal((await intents()).length, 0);
  });
  await check("an invalid live native projection clears its failed surface and reports its precise UI error rather than throwing from animation frames", async () => {
    await projectedMinimap();
    await patch(() => {
      window.suppliedIcon.tileX++;
      window.component.ui.resize(1920, 1080);
    });
    assert.equal(await page.evaluate(() => window.getUiMinimapStatus(window.component.ui)), null);
    assert.match(await page.getByRole("status").innerText(), /ui.minimap.identity/);
    assert.deepEqual(await page.evaluate(() => window.component.services.errors.map(error => error.errorId)), ["ui.minimap.identity"]);
    assert.equal((await intents()).length, 0);
  });
  assert.deepEqual(errors, []);
} finally {
  await writeFile(resolve(results, "ui4-component-tests.json"), JSON.stringify({
    scope: "UI4 and revisioned minimap component tests with explicit deterministic services/pixels, not a live server/renderer journey or final fidelity acceptance.",
    browser: browser.version(), cases, errors, finalAcceptance: false,
  }, null, 2) + "\n");
  await browser.close(); await host.close();
  console.log(`${cases.filter(row => row.passed).length}/${cases.length} UI4 component cases passed.`);
}
