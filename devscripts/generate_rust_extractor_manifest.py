#!/usr/bin/env python3
"""Generate the ordered extractor metadata consumed by the Rust registry.

The manifest is deliberately generated from yt-dlp's public extractor
registry. It records every extractor, including embed-only entries that have
no URL matcher, so the Rust side can report native and TODO coverage without
maintaining a second hand-written inventory.
"""

import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))

from yt_dlp.extractor import gen_extractors  # noqa: E402


OUTPUT = ROOT / 'rust' / 'crates' / 'ytdlp-extractor' / 'data' / 'extractors.json'


def patterns_for(value):
    if value is False:
        return []
    if isinstance(value, str):
        return [value]
    return list(value)


def main():
    records = []
    for extractor in gen_extractors():
        extractor_class = type(extractor)
        records.append({
            'key': extractor_class.__name__,
            'name': extractor_class.IE_NAME,
            'module': extractor_class.__module__,
            'class': extractor_class.__name__,
            'working': bool(extractor_class._WORKING),
            'patterns': patterns_for(extractor_class._VALID_URL),
        })

    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT.write_text(
        json.dumps(records, ensure_ascii=False, separators=(',', ':')) + '\n',
        encoding='utf-8',
    )
    print(f'generated {len(records)} extractor records at {OUTPUT}')


if __name__ == '__main__':
    main()
