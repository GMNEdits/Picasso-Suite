"""Checks the AAF and XML files Picasso Suite writes with readers it shares no code with, so CI
notices when an export stops opening elsewhere.

    python3 check_interchange.py aaf  timeline.aaf   # needs pyaaf2 (pip install pyaaf2)
    python3 check_interchange.py xml  timeline.xml   # the standard library's XML parser

AAF: pyaaf2 (the library OpenTimelineIO's AAF adapter uses) opens the file, finds the timeline
(composition) and follows every clip through its master clip to a file source with an address,
as Media Composer does when it links to the media.
XML: the file is parsed as XML and every clip of the sequence is checked the way Final Cut Pro 7
and Premiere read one: a file with an address (described once, then named by id), a place on the
timeline and in the file, and a frame rate.
"""

import sys
import xml.etree.ElementTree as ET


def fail(message):
    sys.exit("FAILED: " + message)


def check_aaf(path):
    import aaf2

    with aaf2.open(path, "r") as f:
        compositions = list(f.content.toplevel()) or list(f.content.compositionmobs())
        if not compositions:
            fail("no composition")
        top = compositions[0]
        clips = linked = 0
        for slot in top.slots:
            if slot.media_kind not in ("Picture", "Sound"):
                continue
            for c in slot.segment.components:
                if c.__class__.__name__ != "SourceClip":
                    continue
                clips += 1
                url = None
                for step in c.walk():
                    mob = step.mob
                    descriptor = getattr(mob, "descriptor", None) if mob is not None else None
                    if descriptor is not None and "Locator" in descriptor.keys():
                        for loc in descriptor["Locator"]:
                            url = url or loc["URLString"].value
                if url is None:
                    fail("clip %d of slot %d leads to no file address" % (clips, slot.slot_id))
                linked += 1
        if clips == 0:
            fail("the composition has no clips")
        print("AAF: %r, %d clips, every one linked to its file" % (top.name, linked))


def check_xml(path):
    root = ET.parse(path).getroot()
    if root.tag != "xmeml":
        fail("the root is <%s>, not <xmeml>" % root.tag)
    sequence = root.find(".//sequence")
    if sequence is None or sequence.find("rate/timebase") is None:
        fail("no sequence with a frame rate")
    described = {}
    clips = 0
    for track in sequence.iterfind("media/*/track"):
        for item in track.iterfind("clipitem"):
            clips += 1
            name = item.findtext("name", "?")
            f = item.find("file")
            if f is None:
                fail("clip %r has no file" % name)
            fid = f.get("id")
            if f.find("pathurl") is not None:
                if not f.findtext("pathurl", "").startswith("file://"):
                    fail("clip %r's file address is not a file:// URL" % name)
                described[fid] = True
            elif fid not in described:
                fail("clip %r names file %r before it is described" % (name, fid))
            numbers = {}
            for key in ("start", "end", "in", "out"):
                text = item.findtext(key)
                if text is None:
                    fail("clip %r has no <%s>" % (name, key))
                numbers[key] = int(text)
            if numbers["start"] >= 0 and numbers["end"] >= 0 and numbers["end"] <= numbers["start"]:
                fail("clip %r ends before it starts" % name)
            if numbers["out"] <= numbers["in"]:
                fail("clip %r's out point is not after its in point" % name)
    if clips == 0:
        fail("the sequence has no clips")
    print("XML: %r, %d clips, %d files" % (sequence.findtext("name"), clips, len(described)))


if __name__ == "__main__":
    if len(sys.argv) != 3 or sys.argv[1] not in ("aaf", "xml"):
        sys.exit(__doc__)
    {"aaf": check_aaf, "xml": check_xml}[sys.argv[1]](sys.argv[2])
