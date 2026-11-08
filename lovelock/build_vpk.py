#!/usr/bin/env python3
"""Build lovelock/dist/pak01_dir.vpk: Lovelock's HUD addon plus the scripts in lovelock/hud/.

    python3 lovelock/build_vpk.py path/to/lovelock/pak01_dir.vpk [out.vpk]

Lovelock's addon replaces panorama/layout/citadel_hud_top_bar.vxml_c (which includes
its script, death_http_bridge.vjs_c). Each hud/*.js is an independent IIFE appended to
that compiled script, so no Source 2 tooling is needed: a .vjs_c stores the JavaScript
as raw text in its trailing DATA block.
"""
import glob, hashlib, os, struct, sys, zlib

HERE = os.path.dirname(os.path.abspath(__file__))
LAYOUT = "panorama/layout/citadel_hud_top_bar.vxml_c"
SCRIPT = "panorama/scripts/death_http_bridge.vjs_c"
EMBEDDED = 0x7FFF  # archive index meaning "data follows the directory tree"


def cstr(f):
    out = bytearray()
    while (c := f.read(1)) not in (b"\0", b""):
        out += c
    return out.decode("utf8", "replace")


def read_vpk(path):
    """Files of a single-file VPK (v1 or v2) as {path: bytes}."""
    files = {}
    with open(path, "rb") as f:
        signature, version = struct.unpack("<II", f.read(8))
        if signature != 0x55AA1234:
            sys.exit(f"{path}: not a VPK")
        tree_size = struct.unpack("<I", f.read(4))[0]
        if version == 2:
            f.read(16)
        data_base = f.tell() + tree_size
        entries = []
        while ext := cstr(f):
            while folder := cstr(f):
                while name := cstr(f):
                    _crc, preload_len, archive, offset, length, _end = struct.unpack("<IHHIIH", f.read(18))
                    preload = f.read(preload_len)
                    full = (f"{folder}/" if folder != " " else "") + f"{name}.{ext}"
                    if archive != EMBEDDED:
                        sys.exit(f"{path}: {full} lives in a chunk file; only single-file VPKs are supported")
                    entries.append((full, preload, offset, length))
        for full, preload, offset, length in entries:
            f.seek(data_base + offset)
            files[full] = preload + f.read(length)
    return files


def write_vpk(files, path):
    """Single-file VPK v2 with the tree/archive/whole-file MD5 section Source 2 writers emit."""
    tree, blob = bytearray(), bytearray()
    by_ext = {}
    for name in sorted(files):
        folder, base = os.path.split(name)
        stem, ext = base.rsplit(".", 1)
        by_ext.setdefault(ext, {}).setdefault(folder or " ", []).append((stem, name))
    for ext, folders in by_ext.items():
        tree += ext.encode() + b"\0"
        for folder, items in folders.items():
            tree += folder.encode() + b"\0"
            for stem, name in items:
                data = files[name]
                tree += stem.encode() + b"\0"
                tree += struct.pack("<IHHIIH", zlib.crc32(data), 0, EMBEDDED, len(blob), len(data), 0xFFFF)
                blob += data
            tree += b"\0"
        tree += b"\0"
    tree += b"\0"
    body = struct.pack("<7I", 0x55AA1234, 2, len(tree), len(blob), 0, 48, 0) + tree + blob
    tree_md5, archive_md5 = hashlib.md5(tree).digest(), hashlib.md5(b"").digest()
    whole_md5 = hashlib.md5(body + tree_md5 + archive_md5).digest()
    os.makedirs(os.path.dirname(os.path.abspath(path)), exist_ok=True)
    with open(path, "wb") as f:
        f.write(body + tree_md5 + archive_md5 + whole_md5)


def append_to_data(resource, extra):
    """Append bytes to a compiled resource's trailing DATA block, fixing its sizes."""
    _, _, _, block_offset, block_count = struct.unpack_from("<IHHII", resource, 0)
    last = 8 + block_offset + 12 * (block_count - 1)
    relative, size = struct.unpack_from("<II", resource, last + 4)
    if resource[last:last + 4] != b"DATA" or last + 4 + relative + size != len(resource):
        sys.exit("unexpected resource layout: DATA is not the trailing block")
    out = bytearray(resource) + extra
    struct.pack_into("<I", out, last + 8, size + len(extra))
    struct.pack_into("<I", out, 0, len(out))
    return bytes(out)


def main():
    if len(sys.argv) < 2:
        sys.exit(__doc__)
    source = read_vpk(sys.argv[1])
    out_path = sys.argv[2] if len(sys.argv) > 2 else os.path.join(HERE, "dist", "pak01_dir.vpk")
    missing = [name for name in (LAYOUT, SCRIPT) if name not in source]
    if missing:
        sys.exit(f"{sys.argv[1]} is not Lovelock's addon (missing {missing})")
    extra = b""
    for script in sorted(glob.glob(os.path.join(HERE, "hud", "*.js"))):
        name = os.path.basename(script)
        extra += f"\n// ---- deadass hud/{name} ----\n".encode() + open(script, "rb").read()
        print("appending", name)
    write_vpk({LAYOUT: source[LAYOUT], SCRIPT: append_to_data(source[SCRIPT], extra)}, out_path)
    print(out_path, os.path.getsize(out_path))


if __name__ == "__main__":
    main()
