#!/usr/bin/env python3
import argparse
import io
import sys
import xml.etree.ElementTree as ET
from pathlib import Path

def eprint(*args, **kwargs):
    print(*args, file=sys.stderr, **kwargs)

def main(webui_out_path, outpath: Path):
    generated_files = []
    for dirpath, _, filenames in webui_out_path.walk(on_error=print):
        for filename in filenames:
            generated_files.append(dirpath / filename)

    root = ET.Element("gresources")

    ui_gresource = ET.SubElement(root, "gresource", attrib={"prefix": "/com/obscura/vpn/web-ui"})

    for path in sorted(generated_files):
        relpath = str(path.relative_to(webui_out_path))
        file_elem = ET.SubElement(ui_gresource, "file", attrib={"alias": relpath})
        file_elem.text = str(path.absolute()) # TODO

    tree = ET.ElementTree(root)
    ET.indent(tree, space="  ")

    # Only write new bytes if changed, so that mtime-based change detection in build tools work
    existing_bytes = outpath.read_bytes() if (outpath is not None and outpath.is_file()) else None

    new_bytes_obj = io.BytesIO()
    tree.write(new_bytes_obj, encoding="UTF-8", xml_declaration=True)
    new_bytes = new_bytes_obj.getvalue()

    if existing_bytes != new_bytes:
        eprint("Changed, writing")
        outpath.write_bytes(new_bytes)
    else:
        eprint("Not changed")


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("webui_outdir", type=Path)
    parser.add_argument("out_path", type=Path, nargs="?", default=None)
    args = parser.parse_args()

    webui_out_path = args.webui_outdir
    main(webui_out_path, args.out_path)
