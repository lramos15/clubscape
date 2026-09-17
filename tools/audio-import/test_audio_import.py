from array import array
import gzip
import hashlib
import json
import math
import os
from pathlib import Path
import shutil
import struct
import subprocess
import sys
import unittest
from unittest.mock import patch
import uuid
import wave

import audio_import as importer
from browser_oracles import models
from codec import Flac
import midi
from signal_analysis import fft, measure
from source_map import effect_bounds, sequence_events


def smf(events, division=96, track_count=1):
    return b"MThd" + struct.pack(">IHHH", 6, 0, track_count, division) + b"MTrk" + struct.pack(">I", len(events)) + events


NORMAL_EVENTS = b"\x00\xff\x51\x03\x07\xa1\x20\x00\x90\x3c\x64\x60\x80\x3c\x00\x00\xff\x2f\x00"


class MidiTests(unittest.TestCase):
    def test_preserves_notes_tempo_and_exact_native_clock(self):
        result = midi.describe(smf(NORMAL_EVENTS))
        self.assertEqual(result["note_on_count"], 1)
        self.assertEqual(result["end_tick"], 96)
        self.assertEqual(result["duration_seconds"], 0.5)
        self.assertEqual(result["expected_engine_end_frame"], 48_000_000 // (96_000_000 // 22050) + 1)

    def test_running_channel_status(self):
        data = smf(b"\x00\x90\x3c\x64\x01\x40\x64\x01\x43\x64\x01\xff\x2f\x00")
        self.assertEqual(midi.describe(data)["note_on_count"], 3)

    def test_original_meta_running_status_is_equivalent(self):
        standard = smf(b"\x00\xff\x51\x03\x07\xa1\x20\x00\xff\x2f\x00")
        original = smf(b"\x00\xff\x51\x03\x07\xa1\x20\x00\x2f\x00")
        self.assertTrue(midi.compare_runtime(standard, original)["semantic_events_equal"])
        with self.assertRaises(ValueError):
            midi.parse(original)

    def test_tempo_changes_use_integer_tick_time_units(self):
        events = b"\x00\x90\x3c\x64\x60\xff\x51\x03\x0f\x42\x40\x60\xff\x2f\x00"
        self.assertEqual(midi.describe(smf(events))["duration_seconds"], 1.5)

    def test_changed_note_is_not_equivalent(self):
        original = smf(NORMAL_EVENTS.replace(b"\x3c\x64", b"\x3d\x64"))
        with self.assertRaisesRegex(ValueError, "event streams differ"):
            midi.compare_runtime(smf(NORMAL_EVENTS), original)

    def test_rejects_malformed_inputs(self):
        cases = [
            b"", b"MThd", smf(NORMAL_EVENTS)[:-1],
            smf(b"\x00\x3c\x64"), smf(b"\x80\x80\x80\x80\x00\xff\x2f\x00"),
            smf(b"\x00\x90\x3c\x80\x00\xff\x2f\x00"),
            smf(b"\x00\xff\x51\x02\x01\x02\x00\xff\x2f\x00"),
            smf(b"\x00\xff\x2f\x01\x00"),
            smf(b"\x00\xff\x2f\x00\x00\x90\x3c\x64"),
            smf(NORMAL_EVENTS, division=0), smf(NORMAL_EVENTS, division=0xE700),
            smf(NORMAL_EVENTS, track_count=2), smf(NORMAL_EVENTS) + b"x",
        ]
        for data in cases:
            with self.subTest(data=data[:24]), self.assertRaises(ValueError):
                midi.describe(data)


class SourceParameterTests(unittest.TestCase):
    def test_source_frame_offsets_and_repeat_fields_are_retained(self):
        result = sequence_events({
            "id": 879, "frameLengths": [4, 4, 10, 12, 6, 4],
            "frameSounds": {"3": [{"id": 2735, "loops": 1, "location": 1, "retain": 0, "weight": 100}]},
        })
        self.assertEqual(result[0]["source_frame_start_cycle_sum"], 18)
        self.assertEqual(result[0]["nominal_frame_start_ms"], 360)
        self.assertEqual(result[0]["loops"], 1)
        self.assertFalse(result[0]["wall_clock_enqueue_offset_verified"])

    def test_bad_source_sound_frame_rejected(self):
        with self.assertRaises(ValueError):
            sequence_events({"id": 1, "frameLengths": [4], "frameSounds": {"1": [{"id": 1}]}})

    def test_current_curve_frame_does_not_invent_wall_clock_offset(self):
        event = sequence_events({"id": 13612, "frameLengths": None, "frameSounds": {"1": [{"id": 10983}]}})[0]
        self.assertIsNone(event["nominal_frame_start_ms"])

    def test_source_instrument_offsets_bound_actual_pcm(self):
        definition = {
            "field1006": 20, "field1009": 70,
            "field1008": [None, {
                "field1176": 150, "field1188": 23, "field1187": 7, "field1184": 100,
                "field1180": [20, 0, 0, 0, 0], "field1177": [0] * 5, "field1179": [0] * 5,
            }],
        }
        result = effect_bounds(definition)
        self.assertEqual(result["expected_frames"], 173 * 22050 // 1000)
        self.assertEqual(result["loop_start_ms"], 20)
        self.assertEqual(result["instruments"][0]["echo_delay_ms"], 7)
        self.assertFalse(result["queue_delay_trim_applied"])

    def test_real_tree_chop_duration_is_not_echo_delay(self):
        values = json.loads(gzip.decompress(
            Path("assets/source/osrs/cache2695/collections/sound.json.gz").read_bytes()
        ))
        definition = values["asset.source.osrs.cache2695.sound.2735"]
        result = effect_bounds(definition)
        self.assertEqual(result["expected_frames"], 3307)
        self.assertEqual(result["source_duration_ms"], 150)
        self.assertEqual(result["instruments"][0]["echo_delay_ms"], 0)

    def test_original_silent_choice_keeps_its_weight(self):
        values = json.loads(gzip.decompress(Path("assets/source/osrs/cache2695/collections/sequence.json.gz").read_bytes()))
        events = sequence_events(values["asset.source.osrs.cache2695.sequence.13612"])
        choices = [event for event in events if event["frame"] == 1]
        self.assertEqual(sum(event["weight"] for event in choices), 100)
        self.assertEqual(next(event["weight"] for event in choices if event["id"] == 2411), 74)


class FileCase(unittest.TestCase):
    def setUp(self):
        self.path = Path(".local/audio-import/test-data") / uuid.uuid4().hex
        self.path.mkdir(parents=True)

    def tearDown(self):
        shutil.rmtree(self.path)


class IntegrityTests(FileCase):
    def test_missing_and_corrupt_inputs_fail_closed(self):
        path = self.path / "source.bin"
        expected = {"size_bytes": 4, "sha256": hashlib.sha256(b"data").hexdigest()}
        with self.assertRaisesRegex(ValueError, "Missing required input"):
            importer.verify(path, expected)
        path.write_bytes(b"data")
        importer.verify(path, expected)
        path.write_bytes(b"date")
        with self.assertRaisesRegex(ValueError, "integrity mismatch"):
            importer.verify(path, expected)
        with self.assertRaises(ValueError):
            importer.verify(path, {**expected, "size_bytes": 8})

    def test_corrupt_existing_input_is_not_replaced(self):
        source, destination = self.path / "source.bin", self.path / "copy.bin"
        source.write_bytes(b"source")
        destination.write_bytes(b"broken")
        with self.assertRaises(ValueError):
            importer.copy_verified(source, destination, importer.file_record(source))
        self.assertEqual(destination.read_bytes(), b"broken")

    def test_paths_cannot_escape_declared_roots(self):
        for path in ("../outside", "/absolute", "one/../../outside", "a\\..\\outside", ""):
            with self.subTest(path=path), self.assertRaises(ValueError):
                importer.checked_path(self.path, path)
        (self.path / "escape").symlink_to(Path.cwd())
        with self.assertRaises(ValueError):
            importer.checked_path(self.path, "escape/prompt.md")

    def test_unknown_or_missing_staged_file_rejected(self):
        inputs = self.path / "inputs"
        inputs.mkdir()
        source = inputs / "verified.bin"
        source.write_bytes(b"verified")
        prepared = {
            "request": importer.file_record(importer.RESEARCH / "request.json"),
            "dependencies": importer.file_record(importer.TOOLS / "dependencies.json"),
            "inputs": {"verified.bin": importer.file_record(source, "verified.bin")},
        }
        importer.write_json(self.path / "prepared.json", prepared)
        with patch.object(importer, "WORK", self.path):
            importer.verify_inputs()
            source.unlink()
            with self.assertRaisesRegex(ValueError, "Unexpected/missing"):
                importer.verify_inputs()
            source.write_bytes(b"verified")
            (inputs / "unlocked.bin").write_bytes(b"not authorized")
            with self.assertRaisesRegex(ValueError, "Unexpected/missing"):
                importer.verify_inputs()

    def test_missing_input_command_exits_nonzero(self):
        source = self.path / "missing.bin"
        code = (
            "import sys;sys.path.insert(0,'tools/audio-import');"
            "from audio_import import verify;"
            f"verify({str(source)!r},{{'sha256':'00'}})"
        )
        result = subprocess.run([sys.executable, "-c", code], capture_output=True, text=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Missing required input", result.stderr)


class SignalTests(unittest.TestCase):
    def test_codec_float_conversion_models_are_exact_not_tolerance_fits(self):
        samples = array("h", [-32768, -1, 0, 1, 1075, 32767])
        oracles = models(samples, 1, 16)
        self.assertEqual(len(oracles), 2)
        reciprocal = array("f", [1 / 32767])[0]
        expected = array("f", (value * reciprocal if value > 0 else value / 32768 for value in samples))
        self.assertEqual(oracles[1]["float32_channel_sha256"], [hashlib.sha256(expected.tobytes()).hexdigest()])
        self.assertNotEqual(oracles[0]["float32_channel_sha256"], oracles[1]["float32_channel_sha256"])

    def test_fft_detects_known_bin(self):
        values = [math.sin(2 * math.pi * 7 * i / 128) for i in range(128)]
        spectrum = fft(values)
        self.assertAlmostEqual(abs(spectrum[7]), 64, places=8)
        self.assertLess(abs(spectrum[6]), 1e-8)

    def test_invalid_fft_length(self):
        with self.assertRaises(ValueError):
            fft([0, 1, 2])

    def test_empty_silent_or_clipped_output_rejected(self):
        for samples in (array("h"), array("h", [0] * 100), array("h", [-32768, 100]), array("h", [32767, -100])):
            with self.subTest(length=len(samples)), self.assertRaises(ValueError):
                measure(samples, 22050, 1)

    def test_measures_actual_pcm_not_header(self):
        samples = array("h", [int(1000 * math.sin(2 * math.pi * 13 * i / 2048)) for i in range(4096)])
        result = measure(samples, 22050, 1)
        self.assertEqual(result["frames"], 4096)
        self.assertGreater(result["distinct_amplitudes"], 500)
        self.assertAlmostEqual(result["peak"], 1000 / 32768)
        self.assertEqual(result["full_scale_samples"], 0)
        self.assertGreater(len(result["spectral_windows"]), 0)


class CodecTests(FileCase):
    def test_music_and_sfx_packaging_are_exactly_reversible(self):
        codec = Flac()
        source = self.path / "fixture.wav"
        original = array("h", [-32768, -12345, -1, 0, 1, 6789, 32767] * 20)
        with wave.open(str(source), "wb") as output:
            output.setnchannels(1)
            output.setsampwidth(2)
            output.setframerate(22050)
            output.writeframes(original.tobytes())
        for effect in (False, True):
            data, info, metadata = codec.encode(source, self.path / f"{effect}.flac", effect=effect)
            restored = array("h", (value >> 15 for value in data)) if effect else data
            self.assertEqual(restored, original)
            self.assertEqual(info.frames, len(original))
            self.assertTrue(metadata["lossless_round_trip_verified"])
            self.assertTrue(metadata["gain_exactly_reversible"])
            if effect:
                self.assertEqual(measure(data, info.samplerate, info.channels)["peak"], 0.5)

    def test_corrupt_audio_container_rejected(self):
        path = self.path / "broken.flac"
        path.write_bytes(b"fLaC\0\0\0\x22" + b"\0" * 12)
        with self.assertRaises(ValueError):
            Flac().read(path)


@unittest.skipUnless(os.environ.get("AUDIO_IMPORT_INTEGRATION") == "1", "set AUDIO_IMPORT_INTEGRATION=1 after prepare/convert")
class OriginalRuntimeTests(FileCase):
    def test_original_full_track_jingle_and_effects_repeat_exactly(self):
        importer.verify_inputs()
        manifest = json.loads(importer.MANIFEST.read_text())
        selected = [
            record for record in manifest["assets"]
            if (record["source_index"], record["source_group"]) in {
                (6, 0), (11, 154), (4, 2735), (4, 3220), (4, 2603), (4, 3790), (4, 220), (4, 2725),
            }
        ]
        self.assertEqual(len(selected), 8)
        jobs = {"tracks": [], "sound_ids": []}
        for record in selected:
            if record["kind"] == "sfx":
                jobs["sound_ids"].append(record["source_group"])
            else:
                jobs["tracks"].append({
                    "index": record["source_index"], "group": record["source_group"],
                    "expected_engine_end_frame": record["loop"]["source_engine_end_frame"],
                })
        job_path = self.path / "jobs.json"
        importer.write_json(job_path, jobs)
        output = self.path / "native"
        importer.run_java(importer.locks(), "SourceAudio", importer.WORK / "inputs", job_path, output)
        for record in selected:
            report = json.loads((output / f"{record['kind']}-{record['source_group']}.json").read_text())
            self.assertEqual(report["pcm_s16le_sha256"], record["encoding"]["source_pcm_s16le_sha256"])
            self.assertEqual(report["frames"], record["signal"]["frames"])


if __name__ == "__main__":
    unittest.main()
