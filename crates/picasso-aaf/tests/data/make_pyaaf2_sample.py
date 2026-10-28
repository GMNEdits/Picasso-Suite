"""Writes pyaaf2-edit.aaf, the AAF file `tests/pyaaf2.rs` reads: an edit as Avid Media Composer lays
one out, written by pyaaf2 (MIT, https://github.com/markreidvfx/pyaaf2), which does not share any
code with this crate, so the reader is tried against a file it did not write.

    python -m pip install pyaaf2
    python make_pyaaf2_sample.py

The composition (top level, 25 fps):
  V1: shot A (50 frames from 10), a 10-frame dissolve cut 5 frames in, shot B (40 from 0),
      a 25-frame filler, shot A again inside an effect (30 from 100), a nested composition.
  A1: the sound file, 50 frames from frame 10 (its master's slot counts 48 kHz samples).
Each clip goes master mob -> file source mob (with a network locator) -> tape source mob whose
timecode starts at 10:00:00:00, as Media Composer's own files do.
"""

import os

import aaf2

RATE = 25
HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "pyaaf2-edit.aaf")


def tape(f, name, kind, rate):
    mob = f.create.SourceMob()
    f.content.mobs.append(mob)
    mob.create_tape_slots(name, rate, RATE, media_kind=kind)
    return mob


def picture_file(f, name, url, length, tape_mob):
    mob = f.create.SourceMob()
    mob.name = name
    d = f.create.CDCIDescriptor()
    d["SampleRate"].value = RATE
    d["Length"].value = length
    d["StoredWidth"].value = 1920
    d["StoredHeight"].value = 1080
    d["ComponentWidth"].value = 8
    d["HorizontalSubsampling"].value = 2
    d["FrameLayout"].value = "FullFrame"
    d["VideoLineMap"].value = [42, 0]
    d["ImageAspectRatio"].value = "16/9"
    loc = f.create.NetworkLocator()
    loc["URLString"].value = url
    d["Locator"].append(loc)
    mob.descriptor = d
    f.content.mobs.append(mob)
    slot = mob.create_timeline_slot(RATE, slot_id=1)
    # Into the tape at its timecode, 10:00:00:00.
    slot.segment = tape_mob.create_source_clip(1, start=10 * 3600 * RATE, length=length)
    return mob


def sound_file(f, name, url, samples, tape_mob):
    mob = f.create.SourceMob()
    mob.name = name
    d = f.create.PCMDescriptor()
    d["SampleRate"].value = 48000
    d["AudioSamplingRate"].value = 48000
    d["Channels"].value = 2
    d["QuantizationBits"].value = 24
    d["BlockAlign"].value = 6
    d["AverageBPS"].value = 48000 * 6
    d["Length"].value = samples
    loc = f.create.NetworkLocator()
    loc["URLString"].value = url
    d["Locator"].append(loc)
    mob.descriptor = d
    f.content.mobs.append(mob)
    slot = mob.create_timeline_slot(48000, slot_id=1)
    slot.segment = tape_mob.create_source_clip(1, start=10 * 3600 * 48000, length=samples)
    return mob


def master(f, name, file_mob, kind, rate, length):
    mob = f.create.MasterMob(name)
    f.content.mobs.append(mob)
    slot = mob.create_timeline_slot(rate, slot_id=1)
    clip = file_mob.create_source_clip(1, start=0, length=length)
    clip.media_kind = kind
    slot.segment = clip
    return mob


def opdef(f, auid, name):
    """The operation definition `auid`, registered in the file's dictionary."""
    d = f.create.OperationDef(auid, name)
    d.media_kind = "picture"
    d["NumberInputs"].value = 2
    f.dictionary.register_def(d)
    return d


def main():
    with aaf2.open(OUT, "w") as f:
        video_tape = tape(f, "TAPE 1", "picture", RATE)
        sound_tape = tape(f, "TAPE 1 SOUND", "sound", 48000)
        a_file = picture_file(f, "shot A", "file:///C:/Media/shot%20A.mov", 600, video_tape)
        b_file = picture_file(f, "shot B", "file:///Volumes/Media/B.mxf", 600, video_tape)
        s_file = sound_file(f, "sound", "file:///C:/Media/sound.wav", 48000 * 60, sound_tape)
        a = master(f, "Shot A", a_file, "picture", RATE, 600)
        b = master(f, "Shot B", b_file, "picture", RATE, 600)
        s = master(f, "Sound", s_file, "sound", 48000, 48000 * 60)

        nested = f.create.CompositionMob("Nested")
        nested.usage = "Usage_LowerLevel"
        f.content.mobs.append(nested)
        n_slot = nested.create_picture_slot(RATE)
        n_slot.segment.components.append(b.create_source_clip(1, start=0, length=20))

        comp = f.create.CompositionMob("Edit from pyaaf2")
        comp.usage = "Usage_TopLevel"
        f.content.mobs.append(comp)
        v1 = comp.create_picture_slot(RATE)
        seq = v1.segment
        seq.components.append(a.create_source_clip(1, start=10, length=50))
        t = f.create.Transition("picture", 10)
        t["CutPoint"].value = 5
        # pyaaf2's `f.create.X(...)` keeps X in shared state, so no `f.create` may run inside
        # another one's arguments: the definition is made first.
        dissolve = opdef(f, "0c3bea40-fc05-11d2-8a29-0050040ef7d2", "VideoDissolve")
        t["OperationGroup"].value = f.create.OperationGroup(dissolve, 10, "picture")
        seq.components.append(t)
        seq.components.append(b.create_source_clip(1, start=0, length=40))
        seq.components.append(f.create.Filler("picture", 25))
        effect = opdef(f, "b1c0fd61-4a8b-4d1b-9a3e-2f0d6c5a7e11", "Blur")
        group = f.create.OperationGroup(effect, 30, "picture")
        group.segments.append(a.create_source_clip(1, start=100, length=30))
        seq.components.append(group)
        seq.components.append(nested.create_source_clip(1, start=0, length=20))

        a1 = comp.create_sound_slot(RATE)
        # The master's sound slot counts samples: frame 10 at 25 fps is sample 19200.
        sound = s.create_source_clip(1, start=19200, length=50)
        a1.segment.components.append(sound)
    print("wrote", OUT, os.path.getsize(OUT), "bytes")


if __name__ == "__main__":
    main()
