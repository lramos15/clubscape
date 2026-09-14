#!/usr/bin/env node
// Decode public source media and exercise the review gallery, never a game client.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { createServer } from 'node:http';
import { readFile, writeFile, mkdir } from 'node:fs/promises';
import { resolve, dirname, extname, relative, sep } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const argv = process.argv.slice(2);
const value = (key) => {
  const index = argv.indexOf(key);
  assert(index >= 0 && argv[index + 1], `${key} requires an existing local path`);
  return resolve(argv[index + 1]);
};
const chrome = value('--chrome');
const playwrightPath = value('--playwright');
const { chromium } = await import(pathToFileURL(playwrightPath).href);
const hash = (bytes) => createHash('sha256').update(bytes).digest('hex');
const media = JSON.parse(await readFile(resolve(root, 'research/reference-pack/v1/public-media.json')));
const types = {
  '.html': 'text/html; charset=utf-8', '.png': 'image/png', '.jpg': 'image/jpeg',
  '.gif': 'image/gif', '.mp4': 'video/mp4', '.flac': 'audio/flac',
  '.json': 'application/json', '.css': 'text/css', '.js': 'text/javascript',
};
const requests = [];
const server = createServer(async (request, response) => {
  const pathname = decodeURIComponent(new URL(request.url, 'http://localhost').pathname);
  if (pathname === '/favicon.ico') {
    response.writeHead(204); response.end(); return;
  }
  const path = resolve(root, '.' + pathname);
  if (relative(root, path).startsWith('..' + sep) || path === root) {
    response.writeHead(403); response.end('Outside source gallery'); return;
  }
  if (!['assets/reference/', 'assets/source/osrs/audio-runtime/', 'research/reference-pack/']
      .some((prefix) => relative(root, path).startsWith(prefix))) {
    response.writeHead(403); response.end('Outside reference evidence'); return;
  }
  try {
    const bytes = await readFile(path);
    response.writeHead(200, { 'Content-Type': types[extname(path)] ?? 'application/octet-stream' });
    response.end(bytes);
  } catch (error) {
    if (error.code !== 'ENOENT' && error.code !== 'EISDIR') throw error;
    requests.push({ path: request.url, status: 404 });
    response.writeHead(404); response.end('Missing source evidence');
  }
});
await new Promise((done) => server.listen(0, '127.0.0.1', done));
const origin = `http://127.0.0.1:${server.address().port}`;
let browser;
try {
  assert.equal((await fetch(`${origin}/research/reference-pack/v1/public-media.json`)).status, 200);
  browser = await chromium.launch({ executablePath: chrome, headless: true, chromiumSandbox: true });
  const page = await browser.newPage({ viewport: { width: 1920, height: 1080 }, deviceScaleFactor: 1 });
  const errors = [];
  page.on('pageerror', (error) => errors.push(String(error)));
  await page.goto(`${origin}/research/reference-pack/v1/public-media.json`);
  const recordings = [];
  for (const asset of media.filter((item) => item.decoded.format === 'MP4')) {
    const bytes = await readFile(resolve(root, asset.path));
    assert.equal(hash(bytes), asset.sha256);
    const metadata = await page.evaluate(async (url) => {
      document.body.replaceChildren();
      const video = document.createElement('video');
      video.muted = true;
      video.preload = 'auto';
      document.body.append(video);
      await new Promise((done, fail) => {
        video.onloadeddata = done;
        video.onerror = () => fail(new Error(`Video decode failed: ${video.error?.message}`));
        video.src = url;
      });
      return { width: video.videoWidth, height: video.videoHeight, duration: video.duration,
        browserHasAudioTracksApi: 'audioTracks' in video };
    }, `${origin}/${asset.path}`);
    assert.deepEqual([metadata.width, metadata.height], asset.decoded.dimensions);
    assert(Math.abs(metadata.duration - asset.imageinfo.duration) < 0.05);
    const frames = [];
    for (const seconds of [0, 5, 10, 15, 20, 25, 30, 35, 40]) {
      const frame = await page.evaluate(async (target) => {
        const video = document.querySelector('video');
        if (Math.abs(video.currentTime - target) > 0.0001) {
          await new Promise((done, fail) => {
            video.onseeked = done;
            video.onerror = () => fail(new Error(`Seek failed: ${video.error?.message}`));
            video.currentTime = target;
          });
        }
        const canvas = document.createElement('canvas');
        canvas.width = video.videoWidth;
        canvas.height = video.videoHeight;
        const context = canvas.getContext('2d', { willReadFrequently: true });
        context.drawImage(video, 0, 0);
        const pixels = context.getImageData(0, 0, canvas.width, canvas.height).data;
        const sample = new Set();
        for (let i = 0; i < pixels.length; i += 64) sample.add(`${pixels[i]},${pixels[i + 1]},${pixels[i + 2]}`);
        return { png: canvas.toDataURL('image/png').split(',')[1],
          requested_time_seconds: target, seek_time_seconds: video.currentTime,
          sampled_color_count: sample.size };
      }, seconds);
      assert(frame.sampled_color_count > 32, `Blank decoded source frame ${seconds}`);
      const path = `assets/reference/wiki/recording-frames/music-transition-${seconds}s.png`;
      const png = Buffer.from(frame.png, 'base64');
      await mkdir(dirname(resolve(root, path)), { recursive: true });
      await writeFile(resolve(root, path), png);
      delete frame.png;
      frames.push({ ...frame, path, sha256: hash(png), size_bytes: png.length,
        dimensions: asset.decoded.dimensions,
        role: 'decoded_public_recording_frame',
        source_asset_id: asset.id,
        exact_frame_presentation_timestamp: null,
        timing_note: 'HTMLVideoElement seek time, not a measured source-client animation clock.' });
    }
    const audio = await page.evaluate(async (url) => {
      const bytes = await (await fetch(url)).arrayBuffer();
      const context = new AudioContext({ sampleRate: 48000 });
      try {
        const buffer = await context.decodeAudioData(bytes);
        const channels = [];
        for (let i = 0; i < buffer.numberOfChannels; i++) {
          const data = buffer.getChannelData(i);
          let sum = 0;
          let peak = 0;
          for (const value of data) { sum += value * value; peak = Math.max(peak, Math.abs(value)); }
          const sha = await crypto.subtle.digest('SHA-256', data);
          channels.push({ peak, rms: Math.sqrt(sum / data.length),
            float32_sha256: [...new Uint8Array(sha)].map((b) => b.toString(16).padStart(2, '0')).join('') });
        }
        return { frames: buffer.length, sample_rate: buffer.sampleRate,
          channels, duration: buffer.duration, audible_playback_observed: false,
          note: 'Actual decoded recording audio at explicitly requested 48000 Hz; no source mixer calibration claim.' };
      } finally { await context.close(); }
    }, `${origin}/${asset.path}`);
    assert(audio.channels.length > 0 && audio.channels.some((channel) => channel.peak > 0.01));
    recordings.push({ source_asset_id: asset.id, source_sha256: asset.sha256, metadata, audio, frames });
  }
  const report = {
    schema_version: 1, evidence_scope: 'Public source-media decode and reference-gallery usability only',
    observed_at: new Date().toISOString(), browser: await browser.version(),
    executable: chrome, playwright_module: playwrightPath,
    sandbox_enabled: true, headless: true, dpr: 1, recordings,
    mac_chrome_or_edge_run: false, clubscape_renderer_run: false,
    owner_reference_pack_approved: false, final_presentation_accepted: false,
  };
  if (argv.includes('--gallery')) {
    const manifest = JSON.parse(await readFile(resolve(root, 'research/reference-pack/v1/manifest.json')));
    const expectedCaseIds = manifest.cases.map((entry) => entry.id).sort();
    const expectedTutorialIds = expectedCaseIds.filter((id) => id.startsWith('case.tutorial.'));
    report.gallery = [];
    for (const [width, height] of [[1024, 768], [1280, 800], [1920, 1080], [2560, 1440]]) {
      await page.setViewportSize({ width, height });
      await page.goto(`${origin}/research/reference-pack/v1/gallery/index.html`);
      await page.waitForFunction(() => [...document.images].every((image) => image.complete));
      const result = await page.evaluate(() => ({
        broken_images: [...document.images].filter((image) => !image.naturalWidth).map((image) => image.src),
        horizontal_overflow: document.documentElement.scrollWidth > innerWidth,
        case_ids: [...document.querySelectorAll('[data-case-id]')].map((entry) => entry.dataset.caseId).sort(),
        autoplay_media: [...document.querySelectorAll('audio,video')].filter((entry) => entry.autoplay).length,
      }));
      assert.deepEqual(result.broken_images, []);
      assert.equal(result.horizontal_overflow, false);
      assert.deepEqual(result.case_ids, expectedCaseIds);
      assert.equal(result.autoplay_media, 0);
      await page.locator('#search').fill('case.tutorial.');
      const visibleTutorial = await page.evaluate(() =>
        [...document.querySelectorAll('[data-case-id]')].filter((entry) => !entry.hidden)
          .map((entry) => entry.dataset.caseId).sort());
      assert.deepEqual(visibleTutorial, expectedTutorialIds);
      await page.locator('#search').fill('case.ui.equipment');
      assert.equal(await page.locator('[data-case-id]:visible').count(), 1);
      await page.locator('#search').fill('');
      report.gallery.push({ viewport: [width, height], ...result });
    }
  }
  assert.deepEqual(errors, []);
  assert.deepEqual(requests, []);
  await writeFile(resolve(root, 'research/reference-pack/v1/browser-media.json'),
    JSON.stringify(report, null, 2) + '\n');
  console.log(JSON.stringify({ browser: report.browser, decoded_recordings: recordings.length,
    source_frames: recordings.reduce((sum, item) => sum + item.frames.length, 0),
    gallery_viewports: report.gallery?.length ?? 0, source_only: true }));
} finally {
  await browser?.close();
  await new Promise((done) => server.close(done));
}
