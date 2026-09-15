class SourceAudioMonitor extends AudioWorkletProcessor {
  constructor() {
    super();
    this.frames = 0;
    this.nonzero = 0;
    this.peak = 0;
    this.clips = 0;
    this.channels = 0;
    this.capture = null;
    this.port.onmessage = ({ data }) => {
      if (data.kind === "capture") {
        this.capture = { id: data.id, length: data.frames, samples: [], startFrame: null };
      } else if (data.kind === "stats") {
        this.port.postMessage({
          kind: "stats", request: data.request, frames: this.frames, nonzero: this.nonzero,
          peak: this.peak, clips: this.clips, channels: this.channels, currentFrame, currentTime, sampleRate,
        });
      }
    };
  }
  process(inputs, outputs) {
    const input = inputs[0]?.[0];
    for (const output of outputs[0] ?? []) output.fill(0);
    if (!input) return true;
    this.frames += input.length;
    this.channels = Math.max(this.channels, inputs[0].length);
    for (const channel of inputs[0]) {
      for (const sample of channel) {
        if (sample !== 0) this.nonzero++;
        this.peak = Math.max(this.peak, Math.abs(sample));
        if (Math.abs(sample) > 1) this.clips++;
      }
    }
    if (this.capture) {
      const capture = this.capture;
      if (capture.startFrame === null) capture.startFrame = currentFrame;
      for (const value of input) if (capture.samples.length < capture.length) capture.samples.push(value);
      if (capture.samples.length === capture.length) {
        this.port.postMessage({ kind: "capture", ...capture, currentFrame, sampleRate });
        this.capture = null;
      }
    }
    return true;
  }
}
registerProcessor("source-audio-monitor", SourceAudioMonitor);
