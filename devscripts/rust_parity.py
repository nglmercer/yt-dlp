#!/usr/bin/env python3
"""Run offline differential checks against the Rust migration binary.

The Python imports in this script are a development-only behavioral oracle.
This script is never imported or invoked by the Rust product binary.
"""

from __future__ import annotations

import argparse
import contextlib
import io
import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))

from yt_dlp.networking import Request  # noqa: E402
from yt_dlp.options import create_parser, parseOpts  # noqa: E402
from yt_dlp.extractor import gen_extractors  # noqa: E402
from yt_dlp.utils import (  # noqa: E402
    determine_ext,
    determine_protocol,
    float_or_none,
    format_bytes,
    int_or_none,
    parse_iso8601,
    parse_bytes,
    parse_duration,
    str_or_none,
)


DEFAULT_RUST_BINARY = ROOT / 'rust' / 'target' / 'debug' / 'yt-dlp-rs'


def run_rust(binary: Path, operation: str, values: list[object]) -> list[dict[str, object]]:
    requests = [
        json.dumps({'operation': operation, 'input': value}, ensure_ascii=False)
        for value in values
    ]
    try:
        process = subprocess.Popen(
            [str(binary), '--parity-stdio'],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            encoding='utf-8',
        )
    except OSError as error:
        raise RuntimeError(f'could not start Rust binary {binary}: {error}') from error

    stdout, stderr = process.communicate('\n'.join(requests) + '\n')
    if process.returncode:
        raise RuntimeError(
            f'Rust parity process exited with {process.returncode}: {stderr.strip() or "no stderr"}')

    responses = []
    for line_number, line in enumerate(stdout.splitlines(), 1):
        try:
            responses.append(json.loads(line))
        except json.JSONDecodeError as error:
            raise RuntimeError(f'Rust emitted invalid JSON on line {line_number}: {line!r}') from error

    if len(responses) != len(values):
        raise RuntimeError(f'Rust returned {len(responses)} responses for {len(values)} requests')
    return responses


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--rust-bin', type=Path, default=DEFAULT_RUST_BINARY)
    parser.add_argument(
        '--operation',
        choices=(
            'format_bytes', 'parse_bytes', 'parse_duration', 'request_model',
            'cli_options', 'cli_inventory', 'extractor_inventory', 'core_utils'),
        default='format_bytes')
    parser.add_argument('--fixture', type=Path)
    args = parser.parse_args()

    if not args.rust_bin.is_file():
        parser.error(f'Rust binary not found: {args.rust_bin}. Run `make rust-parity` first.')

    fixture = args.fixture or ROOT / 'test' / 'rust_parity' / f'{args.operation}.json'
    values = json.loads(fixture.read_text(encoding='utf-8'))
    if not isinstance(values, list):
        parser.error(f'fixture must contain a JSON list: {fixture}')

    if args.operation == 'format_bytes':
        expected = [format_bytes(value) for value in values]
    elif args.operation == 'parse_bytes':
        expected = [None if (parsed := parse_bytes(value)) is None else str(parsed) for value in values]
    elif args.operation == 'parse_duration':
        expected = [None if (parsed := parse_duration(value)) is None else parsed for value in values]
    elif args.operation == 'request_model':
        expected = []
        for case in values:
            data = case.get('data')
            if isinstance(data, str):
                data = data.encode()
            request = Request(
                case['url'],
                data=data,
                headers=case.get('headers'),
                method=case.get('method'),
            )
            expected.append({
                'url': request.url,
                'method': request.method,
                'data_hex': request.data.hex() if request.data is not None else None,
                'headers': request.headers.sensitive(),
            })
    elif args.operation == 'extractor_inventory':
        extractors = gen_extractors()
        classes = [type(extractor) for extractor in extractors]
        expected = [{
            'count': len(extractors),
            'first_keys': [extractor.__name__ for extractor in classes[:5]],
            'last_keys': [extractor.__name__ for extractor in classes[-5:]],
            'first_names': [extractor.IE_NAME for extractor in classes[:5]],
            'last_names': [extractor.IE_NAME for extractor in classes[-5:]],
            'working_count': sum(bool(extractor._WORKING) for extractor in classes),
            'pattern_count': sum(
                0 if extractor._VALID_URL is False else
                len(extractor._VALID_URL) if isinstance(extractor._VALID_URL, (list, tuple)) else 1
                for extractor in classes),
            'embed_only_count': sum(extractor._VALID_URL is False for extractor in classes),
        }]
    elif args.operation == 'cli_inventory':
        parser = create_parser()
        records = [*parser.option_list]
        for group in parser.option_groups:
            records.extend(group.option_list)
        expected = [{
            'count': len(records),
            'spelling_count': sum(len(option._short_opts) + len(option._long_opts) for option in records),
            'group_count': len(parser.option_groups) + 1,
            'value_option_count': sum(option.nargs is not None or option.type is not None for option in records),
            'callback_option_count': sum(option.action == 'callback' for option in records),
            'destination_count': sum(option.dest is not None for option in records),
            'choice_option_count': sum(bool(option.choices) for option in records),
            'first_aliases': [
                [*option._short_opts, *option._long_opts] for option in records[:5]
            ],
            'last_aliases': [
                [*option._short_opts, *option._long_opts] for option in records[-5:]
            ],
        }]
    elif args.operation == 'core_utils':
        expected = []
        for case in values:
            function = case['function']
            if function == 'determine_ext':
                result = determine_ext(case.get('url'), case.get('default', 'unknown_video'))
            elif function == 'determine_protocol':
                result = determine_protocol(case['info'])
            elif function == 'int_or_none':
                result = int_or_none(
                    case.get('value'),
                    scale=case.get('scale', 1),
                    invscale=case.get('invscale', 1),
                    base=case.get('base'),
                )
            elif function == 'float_or_none':
                result = float_or_none(
                    case.get('value'),
                    scale=case.get('scale', 1),
                    invscale=case.get('invscale', 1),
                )
            elif function == 'str_or_none':
                result = str_or_none(case.get('value'), case.get('default'))
            elif function == 'parse_iso8601':
                result = parse_iso8601(case.get('value'))
            else:
                raise ValueError(f'unknown core utility: {function}')
            expected.append(result)
    else:
        expected = []
        for case in values:
            stdout, stderr = io.StringIO(), io.StringIO()
            with contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
                _, opts, urls = parseOpts(case, ignore_config_files=True)
            expected.append({
                'urls': urls,
                'proxy': opts.proxy,
                'socket_timeout': opts.socket_timeout,
                'no_check_certificate': opts.no_check_certificate,
                'js_runtimes': opts.js_runtimes,
                'remote_components': opts.remote_components,
                'headers': opts.headers,
                'user_agent': opts.user_agent,
                'referer': opts.referer,
                'quiet': opts.quiet,
                'verbose': opts.verbose,
                'no_warnings': opts.no_warnings,
                'simulate': opts.simulate,
                'skip_download': opts.skip_download,
                'format': opts.format,
                'format_sort': opts.format_sort,
                'extractaudio': opts.extractaudio,
                'audioformat': opts.audioformat,
                'audioquality': opts.audioquality,
                'merge_output_format': opts.merge_output_format,
                'remuxvideo': opts.remuxvideo,
                'recodevideo': opts.recodevideo,
                'postprocessor_args': opts.postprocessor_args,
                'keepvideo': opts.keepvideo,
                'nopostoverwrites': opts.nopostoverwrites,
                'ffmpeg_location': opts.ffmpeg_location,
                'sleep_interval_subtitles': opts.sleep_interval_subtitles,
                'sleep_interval_requests': opts.sleep_interval_requests,
                'sleep_interval': opts.sleep_interval,
                'max_sleep_interval': opts.max_sleep_interval,
                'outtmpl': opts.outtmpl,
                'overwrites': opts.overwrites,
                'continue_dl': opts.continue_dl,
                'noplaylist': opts.noplaylist,
                'dumpjson': opts.dumpjson,
                'dump_single_json': opts.dump_single_json,
                'geturl': opts.geturl,
                'gettitle': opts.gettitle,
                'getid': opts.getid,
                'getthumbnail': opts.getthumbnail,
                'getduration': opts.getduration,
                'writeinfojson': opts.writeinfojson,
                'listformats': opts.listformats,
                'batchfile': opts.batchfile,
                'playlist_items': opts.playlist_items,
                'age_limit': opts.age_limit,
                'retries': opts.retries,
                'concurrent_fragments': opts.concurrent_fragment_downloads,
                'ignoreconfig': opts.ignoreconfig,
                'config_locations': opts.config_locations,
            })

    actual = run_rust(args.rust_bin, args.operation, values)
    mismatches = []

    for index, (value, expected_value, response) in enumerate(zip(values, expected, actual, strict=True)):
        if response.get('ok') is not True or response.get('output') != expected_value:
            mismatches.append({
                'index': index,
                'input': value,
                'expected': expected_value,
                'actual': response,
            })

    if mismatches:
        print(json.dumps(mismatches, indent=2, ensure_ascii=False), file=sys.stderr)
        return 1

    print(f'PASS {args.operation}: {len(values)} reference/Rust cases match')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
