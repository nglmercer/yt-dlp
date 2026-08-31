#!/usr/bin/env python3
"""Generate the complete ordered CLI option inventory for the Rust port.

The Rust parser is intentionally implemented in slices, but its coverage must
be measured against the live Python option schema.  This manifest records
option definitions and aliases without serializing executable callbacks or
default objects.
"""

import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))

from yt_dlp.options import create_parser  # noqa: E402


OUTPUT = ROOT / 'rust' / 'crates' / 'ytdlp-cli' / 'data' / 'options.json'


def main():
    parser = create_parser()
    records = []

    def add_options(group_name, options):
        for option in options:
            records.append({
                'group': group_name,
                'aliases': [*option._short_opts, *option._long_opts],
                'dest': option.dest,
                'action': option.action,
                'type': option.type,
                'nargs': option.nargs,
                'choices': list(option.choices) if option.choices else None,
            })

    add_options('global', parser.option_list)
    for group in parser.option_groups:
        add_options(group.title, group.option_list)

    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT.write_text(
        json.dumps(records, ensure_ascii=False, separators=(',', ':')) + '\n',
        encoding='utf-8',
    )
    print(f'generated {len(records)} CLI option records at {OUTPUT}')


if __name__ == '__main__':
    main()
