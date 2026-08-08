#!/usr/bin/env python3
"""Add color_pipeline field to every DrawCtx { ... } literal that has color_mode.

Pattern: a line like `color_mode: <variant>,` (possibly with trailing comma and
whitespace) is followed by a new line `color_pipeline: ColorPipeline::detect(<variant>),`
with the same indentation. The <variant> is captured and reused so the pipeline
matches the color mode at every test site.

Walks the file list from argv[1:]. Mutates in place.
"""
import re
import sys
from pathlib import Path

PAT = re.compile(r"^(\s*)color_mode:\s*([^,\n]+),\s*$")

def patch(path: Path) -> int:
    text = path.read_text()
    out_lines = []
    n = 0
    for line in text.splitlines(keepends=False):
        out_lines.append(line)
        m = PAT.match(line)
        if m:
            indent, expr = m.group(1), m.group(2).rstrip()
            if expr.startswith("crate::runtime::ColorMode::"):
                pipeline_expr = expr.replace("ColorMode::", "ColorPipeline::")
                pipeline_call = f"crate::runtime::{pipeline_expr}::detect({expr})"
            elif expr.startswith("ColorMode::"):
                pipeline_call = f"ColorPipeline::detect({expr})"
            else:
                pipeline_call = f"ColorPipeline::detect({expr})"
            out_lines.append(f"{indent}color_pipeline: {pipeline_call},")
            n += 1
    path.write_text("\n".join(out_lines) + ("\n" if text.endswith("\n") else ""))
    return n

def main() -> int:
    total = 0
    for arg in sys.argv[1:]:
        p = Path(arg)
        if not p.exists():
            print(f"skip (missing): {arg}", file=sys.stderr)
            continue
        n = patch(p)
        print(f"{arg}: +{n} color_pipeline field(s)")
        total += n
    print(f"total: +{total} fields")
    return 0

if __name__ == "__main__":
    sys.exit(main())
