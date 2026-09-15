#!/usr/bin/env python3
"""Publish/verify ONLY the explicitly authorized additive original native audio inputs."""

import argparse
from array import array
import ctypes as C
import hashlib
import importlib.util
import json
import math
import os
from pathlib import Path
import shutil
import subprocess
import sys

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[3]
WORK = Path("tools/browser-audio-tests/native/.run/supplement")
OUT = Path("assets/source/osrs/audio-supplement")
MANIFEST = Path("assets/manifests/osrs/audio-m1-supplement.json")
RESEARCH = Path("research/browser-audio-policy")
BASE = Path("assets/manifests/osrs/audio-runtime.json")
BASE_SHA = "8e6d1eca465f720829599cae0e1c11b7700a9c7162e53dd5ddec36ceb89787aa"
PACK_SHA = "b62e19704e17d3d3e4e819f803ef49ba7cc54034ae407184b423427c65d9674d"
AUTHORITY = "58a40d1194aa5806f2fa8919bfe2877aaa7d9721"
EXPECTED = {(6,64),(6,327),(6,163),(6,145),(11,40),(11,54),(11,58),(11,64),(11,65)}


def digest(path):
    with Path(path).open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def record(path):
    return {"path":str(path),"sha256":digest(path),"size_bytes":Path(path).stat().st_size}


def module(name,path):
    spec=importlib.util.spec_from_file_location(name,path)
    result=importlib.util.module_from_spec(spec)
    spec.loader.exec_module(result)
    return result


def frozen():
    assert digest(BASE)==BASE_SHA
    assert digest("research/reference-pack/v1/manifest.json")==PACK_SHA
    manifest=json.loads(BASE.read_text())
    reference=json.loads(Path("research/reference-pack/v1/audio-reference.json").read_text())
    for entry in manifest["assets"]+reference["reference_templates"]:
        assert digest(entry["path"])==entry["sha256"],entry["path"]
    return manifest


def cache_verified(cache):
    for entry in json.loads(Path("research/current-source/cache-files.json").read_text()):
        source=cache/entry["name"]
        assert source.stat().st_size==entry["size_bytes"] and digest(source)==entry["sha256"],source


def statistics(samples,frames):
    assert len(samples)==frames*2 and samples.typecode=="h"
    peak=max(abs(value) for value in samples)
    assert peak>0
    first=next(i//2 for i,v in enumerate(samples) if v)
    last=(len(samples)-1-next(i for i,v in enumerate(reversed(samples)) if v))//2
    energy=sum(int(v)*v for v in samples)
    return {"sample_rate":22050,"channels":2,"frames":frames,"duration_seconds":frames/22050,
            "first_nonzero_frame":first,"last_nonzero_frame":last,"peak":peak/32768,
            "rms_dbfs":20*math.log10(math.sqrt(energy/len(samples))/32768),
            "full_scale_samples":sum(v in (-32768,32767) for v in samples)}


def encode_unity24(codec, native_wav, output):
    native, decoded = codec.read(native_wav)
    assert native.typecode == "h" and decoded.channels == 2 and decoded.samplerate == 22050
    stored = array("i", (value << 16 for value in native))
    frames = decoded.frames
    info = type(decoded)(frames=frames, samplerate=22050, channels=2, format=0x170003)
    output.parent.mkdir(parents=True, exist_ok=True)
    handle = codec.lib.sf_open(str(output).encode(), 0x20, C.byref(info))
    if not handle:
        raise ValueError(codec.error(None))
    try:
        compression = C.c_double(codec.lock["compression_level"])
        assert codec.lib.sf_command(handle, 0x1301, C.byref(compression), C.sizeof(compression)) == 1
        pointer = (C.c_int32 * len(stored)).from_buffer(stored)
        assert codec.lib.sf_writef_int(handle, pointer, frames) == frames
    finally:
        assert codec.lib.sf_close(handle) == 0
    restored, actual = codec.read(output)
    assert restored == stored and array("h", (v >> 16 for v in restored)) == native
    return native, actual, {
        "codec": "FLAC", "bits_per_sample": 24, "effective_source_bits": 16,
        "gain_numerator": 1, "gain_denominator": 1,
        "source_pcm_s16le_sha256": hashlib.sha256(native.tobytes()).hexdigest(),
        "decoded_pcm_sha256": hashlib.sha256(stored.tobytes()).hexdigest(),
        "decoded_pcm_hash_format": "s32le-left-aligned",
        "lossless_round_trip_verified": True, "gain_exactly_reversible": True,
        "native_full_scale_samples": sum(v in (-32768,32767) for v in native),
        "reason": "Unity-gain lossless bit-container expansion; exact source16 PCM, with power-of-two browser float mapping even at native saturation. No waveform gain patch.",
    }


def verify(codec):
    original=frozen()
    manifest=json.loads(MANIFEST.read_text())
    assert manifest["base_manifest"]["sha256"]==BASE_SHA
    assert manifest["source_pack_sha256"]==PACK_SHA
    assert {(a["source_index"],a["source_group"]) for a in manifest["assets"]}==EXPECTED
    existing={a["asset_id"] for a in original["assets"]}
    for a in manifest["assets"]:
        assert a["asset_id"] not in existing
        path=Path(a["path"])
        assert path.is_relative_to(OUT) and digest(path)==a["sha256"] and path.stat().st_size==a["size_bytes"]
        pcm,info=codec.read(path)
        assert info.frames==a["signal"]["frames"] and info.samplerate==22050 and info.channels==2
        assert hashlib.sha256(pcm.tobytes()).hexdigest()==a["encoding"]["decoded_pcm_sha256"]
        native = array("h",(v>>16 for v in pcm))
        assert hashlib.sha256(native.tobytes()).hexdigest()==a["encoding"]["source_pcm_s16le_sha256"]
        assert sum(v in (-32768,32767) for v in native)==a["signal"]["full_scale_samples"]
        assert a["native_mixer_level"]==255
    print(json.dumps({"result":"verified_additive_original_audio","files":len(manifest["assets"]),
                      "manifest_sha256":digest(MANIFEST),"unchanged_base_files":266}))


def publish(args,codec,midi,native):
    old=frozen()
    cache_verified(args.cache)
    jars=native.artifact_paths(args.reuse)
    for folder in ["classes","home","work","prepared","pass1","pass2","encoded","controls"]:
        (WORK/folder).mkdir(parents=True,exist_ok=True)
    java=["-XX:-UsePerfData","-XX:ActiveProcessorCount=2","-Xmx1g","-Djava.awt.headless=true",
          f"-Duser.home={(WORK/'home').resolve()}",f"-Djava.io.tmpdir={(WORK/'work').resolve()}"]
    source=[Path("tools/audio-import/SourceAudio.java"),Path("tools/audio-import/BindingCache.java"),
            Path("tools/browser-audio-tests/native/AudioSupplement.java")]
    cp=os.pathsep.join(map(str,[WORK/"classes",*jars]))
    subprocess.run(["javac",*["-J"+v for v in java],"--release","17","-proc:none",
                    "-cp",os.pathsep.join(map(str,jars)),"-d",str(WORK/"classes"),*map(str,source)],check=True,timeout=120)

    def run(mode,output,*extra):
        result=subprocess.run(["java",*java,"-cp",cp,"AudioSupplement",mode,str(args.cache),str(output),*map(str,extra)],
                              capture_output=True,text=True,timeout=600)
        (WORK/f"{output.name}-{mode}.log").write_text(result.stdout+result.stderr)
        if result.returncode:raise RuntimeError(result.stderr[-6000:])

    run("prepare",WORK/"prepared")
    prepared=json.loads((WORK/"prepared/prepared.json").read_text())
    jobs=[]
    timings={}
    for item in prepared:
        desc=midi.describe((WORK/"prepared"/item["midi_file"]).read_bytes(),allow_meta_running_status=True)
        timings[item["index"],item["group"]]=desc
        jobs.append({"index":item["index"],"group":item["group"],"expected_engine_end_frame":desc["expected_engine_end_frame"]})
    (WORK/"jobs.json").write_text(json.dumps({"jobs":jobs})+"\n")
    run("render",WORK/"pass1",WORK/"jobs.json")
    run("render",WORK/"pass2",WORK/"jobs.json")
    control_jobs=[{**job,"native_level":level} for job in jobs for level in [44,136]]
    (WORK/"controls.json").write_text(json.dumps({"jobs":control_jobs})+"\n")
    run("render",WORK/"controls",WORK/"controls.json")

    needs=json.loads((RESEARCH/"publication-needs.json").read_text())["requirements"]
    musical=json.loads((RESEARCH/"native-catalog.json").read_text())
    rows=next(c for c in musical["cases"] if c["case"]=="original-music-table44-and-area-group128")["same_area_rows"]
    artifacts={p.name:record(p) for p in jars}
    inputs={"schema_version":1,"runtime_artifacts":artifacts,"tools":[record(p) for p in source],
            "decoder_tools":[record(Path("tools/audio-import/midi.py")),record(Path("tools/audio-import/codec.py"))],
            "publisher":record(Path("tools/browser-audio-tests/native/audio_supplement.py")),"assets":{}}
    assets=[]
    level_comparisons=[]
    for job in jobs:
        index,group=job["index"],job["group"]
        kind="music" if index==6 else "jingle"
        key=f"{kind}-{group}-native255"
        report=json.loads((WORK/"pass1"/f"{key}.json").read_text())
        repeat=json.loads((WORK/"pass2"/f"{key}.json").read_text())
        assert report==repeat,key
        assert digest(WORK/"pass1"/f"{key}.wav")==digest(WORK/"pass2"/f"{key}.wav"),key
        if index==11:
            expected=next(n for n in needs if n["kind"]=="native_mixer_representation" and n["group"]==group)
            assert report["pcm_s16le_sha256"]==expected["required_native_255_pcm_sha256"],key
            assert report["frames"]==expected["frames"] and report["native_device_clipped_samples"]==expected["source_native_clip_samples"],key
        for g in report["source_groups"].values():
            if "file_ids" in g:
                g["file_count"]=len(g.pop("file_ids"))
        inputs["assets"][key]=report
        encoded=WORK/"encoded"/f"{key}.flac"
        samples,info,encoding=encode_unity24(codec,WORK/"pass1"/f"{key}.wav",encoded)
        for level in [44,136]:
            control_key=f"{kind}-{group}-native{level}"
            control_record=json.loads((WORK/"controls"/f"{control_key}.json").read_text())
            control,control_info=codec.read(WORK/"controls"/f"{control_key}.wav")
            assert len(control)==len(samples) and control_info.frames==info.frames
            native_energy=0.0
            candidate_energy=0.0
            max_difference=0.0
            additional_clips=0
            for native_value,published in zip(control,samples):
                candidate=published*level/255
                native_energy+=native_value*native_value
                candidate_energy+=candidate*candidate
                max_difference=max(max_difference,abs(candidate-native_value)/32768)
                if abs(candidate/32768)>1 and native_value not in (-32768,32767):additional_clips+=1
            error_db=10*math.log10(candidate_energy/native_energy) if native_energy else 0
            assert abs(error_db)<=0.25 and additional_clips==0
            level_comparisons.append({"index":index,"group":group,"native_mixer":level,
                "original_native_pcm_sha256":control_record["pcm_s16le_sha256"],
                "gain_error_db":error_db,"maximum_pointwise_difference":max_difference,
                "additional_clip_samples":additional_clips,
                "classification":"Original native mixer level comparison; not a claim of bit-identical re-synthesis from a mixed representation."})
        assert encoding["source_pcm_s16le_sha256"]==report["pcm_s16le_sha256"]
        target=OUT/kind/f"{group}-native255.flac"
        target.parent.mkdir(parents=True,exist_ok=True)
        if target.exists():
            assert digest(target)==digest(encoded),"Refusing to overwrite a different additive original"
        else:
            with target.open("xb") as destination:destination.write(encoded.read_bytes())
        name=next((r["definition"]["columnValues"][1][0] for r in rows if r["definition"]["columnValues"][4][0]==group),None) if index==6 else None
        previous=next((a for a in old["assets"] if a["kind"]==kind and a["source_group"]==group),None)
        timer=next((r["definition"]["columnValues"][3][0] for r in rows if r["definition"]["columnValues"][4][0]==group),None) if index==6 else None
        timing=timings[index,group]
        assets.append({
            "asset_id":f"asset.source.osrs.cache2695.audio-supplement.{kind}.{group}.native255",
            "kind":kind,"name":name or previous["name"],"source_index":index,"source_group":group,"source_file":0,
            "native_mixer_level":255,"base_asset_id":previous["asset_id"] if previous else None,
            **record(target),"encoding":encoding,"signal":statistics(samples,info.frames),
            "loop":{"native_midi_loop":False,"export_native_loop":False,"source_engine_end_frame":timing["expected_engine_end_frame"],
                    "source_end_tick":timing["end_tick"],"release_tail_frames":22050,
                    "source_timer_ticks":timer,"timer_tick_seconds":0.6,
                    "policy":"One original native pass and1s release; no seamless exported-file loop."},
            "source_track":report["source_inputs"][f"{index}/{group}/0"],
            "native_render":{"input_record":key,"pcm_s16le_sha256":report["pcm_s16le_sha256"],
                             "native_eot_block_start":report["native_eot_block_start"],"native_eot_block_end":report["native_eot_block_end"],
                             "pre_device_peak_s24":report["pre_device_peak_s24"],"source_device_saturation_samples":report["native_device_clipped_samples"],
                             "native_startup_percussion_channel":9,"native_startup_percussion_bank":128,
                             "independent_render_passes":2,"repeated_pcm_and_inputs_equal":True},
        })
    input_file=RESEARCH/"supplement-source-inputs.json"
    input_file.write_text(json.dumps(inputs,sort_keys=True,separators=(",",":"))+"\n")
    manifest={
        "schema_version":1,"kind":"original_additive_m1_audio","authority_commit":AUTHORITY,
        "source_pack_sha256":PACK_SHA,"base_manifest":record(BASE),
        "source_selection":"osrs-live-240-cache2695-injected1.12.38-20260913",
        "native_runtime_sha256":artifacts["injected-client-1.12.38.jar"]["sha256"],
        "settings":{"sample_rate":22050,"channels":2,"codec":"FLAC","bits_per_sample":24,"effective_source_bits":16,"native_mixer_level":255,
                    "native_startup_percussion_channel":9,"native_startup_percussion_bank":128,"native_device_block_frames":512,
                    "waveform_gain_patch":False,"limiter":False,"normalization":False},
        "source_input_manifest":record(input_file),"assets":assets,
        "commands":["python3 tools/browser-audio-tests/native/audio_supplement.py publish",
                    "python3 tools/browser-audio-tests/native/audio_supplement.py verify"],
        "frozen_files_modified":False,"reencoded_base_files":0,"new_adaptation":False,
        "acceptance":"Original additive source publication only; running-browser/gameplay/Mac/owner acceptance remains separate.",
    }
    MANIFEST.write_text(json.dumps(manifest,indent=2)+"\n")
    (RESEARCH/"supplement-level-calibration.json").write_text(json.dumps({
        "schema_version":1,"result":"passed","representation_native_level":255,
        "gain_bound_db":0.25,"additional_clip_bound":0,"cases":level_comparisons,
        "frozen_waveforms_modified":False,
    },indent=2)+"\n")
    cache_verified(args.cache)
    frozen()
    verify(codec)


def main():
    if Path.cwd().resolve()!=ROOT:raise ValueError("Run from the assigned worktree root")
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command",choices=["publish","verify"])
    parser.add_argument("--reuse",type=Path,default=Path("../m1-audio-bindings/.local/audio-import"))
    parser.add_argument("--cache",type=Path,default=Path("../m1-runtime-inputs/.local/current-source/cache-2695"))
    args=parser.parse_args()
    native=module("native_audio_policy",Path("tools/browser-audio-tests/native/probe.py"))
    codec=module("source_flac_codec",Path("tools/audio-import/codec.py")).Flac()
    midi=module("source_midi_clock",Path("tools/audio-import/midi.py"))
    if args.command=="publish":publish(args,codec,midi,native)
    else:verify(codec)


if __name__=="__main__":
    main()
