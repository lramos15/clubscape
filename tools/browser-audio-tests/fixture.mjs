import {
  createAudio, observeAudioState, readAudioState, setSourceMusicSelector,
  setSourceAudioScene, setSourceMasterVolume, sourceMusicRegion, sourceAudioDefaults,
} from "/web/audio/index.ts";

const native = {
  contexts: [], starts: [], ends: [], destinations: [], connections: new Map(),
  buffers: new Map(), gestures: [], sequence: 0,
};
const NativeContext = globalThis.AudioContext;
globalThis.AudioContext = class extends NativeContext {
  constructor(options) {
    super(options);
    native.contexts.push(this);
  }
  createBufferSource() {
    const source = super.createBufferSource();
    const start = source.start.bind(source);
    const id = ++native.sequence;
    source.start = (...args) => {
      start(...args);
      native.buffers.set(source.buffer.length, source.buffer);
      native.starts.push({
        id, requestedAt: this.currentTime, when: args[0] ?? 0,
        offset: args[1] ?? 0, duration: args[2] ?? source.buffer.duration,
        frames: source.buffer.length, loop: source.loop, loopEnd: source.loopEnd,
        state: this.state, sampleRate: this.sampleRate, wallTime: performance.now(),
      });
    };
    source.addEventListener("ended", () => {
      native.ends.push({ id, time: this.currentTime, state: this.state, wallTime: performance.now() });
    });
    return source;
  }
};
const connect = AudioNode.prototype.connect;
const disconnect = AudioNode.prototype.disconnect;
AudioNode.prototype.connect = function (...args) {
  const result = connect.apply(this, args);
  const edges = native.connections.get(this) ?? new Set();
  edges.add(args[0]);
  native.connections.set(this, edges);
  if (args[0] === this.context.destination) native.destinations.push(this);
  return result;
};
AudioNode.prototype.disconnect = function (...args) {
  const result = disconnect.apply(this, args);
  if (!args.length) native.connections.delete(this);
  else native.connections.get(this)?.delete(args[0]);
  return result;
};

const errors = [];
const assets = {
  baseUrl: location.origin,
  url: (id) => `${location.origin}/asset/${encodeURIComponent(id)}`,
  image: async () => { throw new Error("This audio fixture does not load images."); },
  json: async (id) => (await fetch(assets.url(id))).json(),
};
let handle;
let stopObserving;
let monitor;
let monitorSink;
let nextCapture = 0;
const captures = new Map();
const statsRequests = new Map();
let lastUnlock = null;
let world = null;
let eventSequence = 0;

function recordError(error) {
  errors.push({
    code: error.code ?? error.name, message: error.message,
    recoverable: error.recoverable, detail: error.detail ?? null,
  });
}

async function initialize() {
  handle = await createAudio(assets, recordError);
  setSourceMusicSelector(handle, null, "classic");
  stopObserving = observeAudioState(handle, (state) => {
    document.querySelector("#status").textContent =
      `${state.contextState}; gesture=${state.pendingGesture}; voices=${state.voices.length}`;
  });
  const context = native.contexts.at(-1);
  const output = native.destinations.findLast((node) => node.context === context);
  await context.audioWorklet.addModule("/monitor.js");
  monitor = new AudioWorkletNode(context, "source-audio-monitor", {
    numberOfInputs: 1, numberOfOutputs: 1, outputChannelCount: [1],
  });
  monitorSink = context.createGain();
  monitorSink.gain.value = 0;
  // This observation branch is silent; the runtime's independent, enabled
  // master -> AudioDestinationNode connection remains intact and is asserted.
  output.connect(monitor);
  monitor.connect(monitorSink);
  monitorSink.connect(context.destination);
  monitor.port.onmessage = ({ data }) => {
    if (data.kind === "capture") {
      captures.get(data.id)?.(data);
      captures.delete(data.id);
    } else if (data.kind === "stats") {
      statsRequests.get(data.request)?.(data);
      statsRequests.delete(data.request);
    }
  };
}

function syntheticWorld(region = "region.osrs.12336") {
  const tiles = {
    "region.osrs.12336": { x: 3094, y: 3107, plane: 0 },
    "region.osrs.12592": { x: 3140, y: 3090, plane: 0 },
    "region.osrs.12436": { x: 3094, y: 9507, plane: 0 },
    "region.osrs.12850": { x: 3222, y: 3218, plane: 0 },
    "region.osrs.12851": { x: 3222, y: 3280, plane: 0 },
  };
  return {
    revision: "1", tick: "1",
    player: {
      id: "player.audio-fixture", displayName: "Synthetic audio test player", appearance: {}, region,
      tile: tiles[region] ?? { x: 3200, y: 3200, plane: 0 }, instance: null,
      inventory: [], equipment: [], skills: [], hitpoints: 10, prayerPoints: 1, runEnergy: 100,
      questPoints: 0, tutorialStage: "fixture-only", tutorialInstruction: "",
      quests: [
        { id: "quest.learning_the_ropes", name: "Learning the Ropes", stage: "test", journal: "", completed: false },
        { id: "quest.cooks_assistant", name: "Cook's Assistant", stage: "test", journal: "", completed: false },
      ],
      unlockedInterfaces: [], activePrayers: [], activity: "idle", animation: "idle", settings: [],
    },
    entities: [], groundItems: [], dialogue: null, bank: null, shop: null, recovery: null, messages: [],
  };
}

function event(overrides = {}) {
  return {
    id: `fixture/${++eventSequence}`, kind: "sound", sourceId: 2735,
    assetId: null, actorId: world?.player.id ?? null, tile: null, sourceCycle: eventSequence,
    payload: { committed: true, repeatCount: 1, delayCycles: 2 }, ...overrides,
  };
}

function update(nextWorld, events = []) {
  world = nextWorld === null ? null : structuredClone(nextWorld);
  handle.update(world === null ? null : deepFreeze(structuredClone(world)), deepFreeze(structuredClone(events)));
}
function deepFreeze(object) {
  if (object && typeof object === "object") {
    Object.freeze(object);
    for (const value of Object.values(object)) deepFreeze(value);
  }
  return object;
}

async function capture(frames, operation) {
  const id = ++nextCapture;
  const promise = new Promise((resolve) => captures.set(id, resolve));
  monitor.port.postMessage({ kind: "capture", id, frames });
  operation?.();
  return promise;
}
function stats() {
  const request = ++nextCapture;
  const promise = new Promise((resolve) => statsRequests.set(request, resolve));
  monitor.port.postMessage({ kind: "stats", request });
  return promise;
}

document.querySelector("#unlock").addEventListener("click", (event) => {
  native.gestures.push({
    control: "unlock", trusted: event.isTrusted, active: navigator.userActivation.isActive,
    input: event.detail === 0 ? "keyboard-or-synthetic" : "mouse",
  });
  lastUnlock = handle.unlock().then(
    () => ({ success: true, state: readAudioState(handle).contextState }),
    (error) => ({ success: false, code: error.code, message: error.message }),
  );
});
document.querySelector("#mute").addEventListener("click", () => handle.mute(true));
document.querySelector("#unmute").addEventListener("click", () => handle.mute(false));
for (const channel of ["music", "effects", "area"]) {
  document.querySelector(`#${channel}`).addEventListener("input", (event) => {
    handle.volume(channel, Number(event.target.value) / 100);
  });
}
document.querySelector("#master").addEventListener("input", (event) => {
  setSourceMasterVolume(handle, Number(event.target.value));
});

window.audioFixture = {
  get handle() { return handle; },
  get context() { return native.contexts.at(-1); },
  get world() { return structuredClone(world); },
  get lastUnlock() { return lastUnlock; },
  errors, native, syntheticWorld, event, update, capture, stats,
  snapshot: () => readAudioState(handle),
  setSourceAudioScene: (scene) => setSourceAudioScene(handle, scene),
  setSourceMasterVolume: (value) => setSourceMasterVolume(handle, value),
  setSourceMusicSelector: (selector, mode = "modern") => setSourceMusicSelector(handle, selector, mode),
  sourceMusicRegion, sourceAudioDefaults,
  originalsConnected: () => native.destinations.some((node) =>
    node.context === native.contexts.at(-1) && node !== monitorSink &&
    native.connections.get(node)?.has(node.context.destination)),
  warm: async (sourceId, extra = {}) => {
    const e = event({ sourceId, ...extra });
    update(world, [e]);
    return e.id;
  },
  dispose: async () => {
    stopObserving?.();
    monitor?.disconnect();
    monitorSink?.disconnect();
    monitor?.port.close();
    await handle?.dispose();
  },
};
window.audioFixture.ready = initialize();
