#!/usr/bin/env python3
"""Exercise real egui input and capture PNG/JSON evidence in an agent-owned session."""
import argparse
import json
from pathlib import Path
import subprocess
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--cli', type=Path, default=Path('target/debug/fyle-cli'))
    parser.add_argument('--output-dir', type=Path, required=True)
    args = parser.parse_args()
    args.output_dir.mkdir(parents=True, exist_ok=True)
    if any(args.output_dir.iterdir()):
        raise SystemExit("Use a new or empty artifact directory")
    cli = str(args.cli.resolve())
    session = None

    def run(*arguments):
        result = subprocess.run([cli, 'gui', *map(str, arguments)],
                                capture_output=True, text=True, timeout=30)
        if result.returncode:
            raise RuntimeError(f"GUI command failed: {arguments}\n{result.stderr}")
        return json.loads(result.stdout)

    def request(*arguments):
        return run(*arguments, '--session', session)

    def capture(name):
        path = (args.output_dir / (name + '.png')).resolve()
        shot = request('screenshot', '--output', path)
        assert path.read_bytes().startswith(b'\x89PNG\r\n\x1a\n')
        assert shot['width'] > 0 and shot['height'] > 0
        (args.output_dir / (name + '.json')).write_text(json.dumps(shot, indent=2))
        return shot['state']

    def click(control_id):
        # Layout can settle after selection or resizing; stale rejection guarantees
        # that the rejected click did nothing, so recapture before retrying.
        for attempt in range(5):
            state = request('state')
            control = next(control for control in state['controls'] if control['id'] == control_id)
            assert control['enabled']
            x, y, width, height = control['rect_pixels']
            try:
                return request('click', '--x', x + width / 2, '--y', y + height / 2,
                               '--snapshot', state['snapshot_id'])
            except RuntimeError as error:
                if 'Stale snapshot' not in str(error) or attempt == 4:
                    raise

    with tempfile.TemporaryDirectory(prefix='fyle-gui-fixture-') as temporary:
        fixture = Path(temporary)
        (fixture / 'alpha.txt').write_text('fixture alpha')
        (fixture / 'beta.txt').write_text('fixture beta')
        (fixture / 'folder').mkdir()
        (fixture / 'engine.rs').write_text('fn main() { /* local peek fixture */ }\n')
        try:
            launched = run('launch', '--path', fixture)
            session = launched['session']
            assert launched['state']['ready']
            capture('01-list')
            state = click('entry:alpha.txt')
            assert state['selected_count'] == 1
            assert next(entry for entry in state['entries'] if entry['name'] == 'alpha.txt')['selected']
            state = click('search')
            state = request('text', '--text', 'alpha', '--snapshot', state['snapshot_id'])
            assert state['query'] == 'alpha'
            assert [entry['name'] for entry in state['entries']] == ['alpha.txt']
            capture('02-filtered')
            request('key', '--key', 'Ctrl+A')
            state = request('key', '--key', 'Backspace')
            assert state['query'] == ''
            state = click('grid')
            assert state['view'] == 'grid'
            capture('03-grid')
            request('action', 'new_folder')
            state = click('dialog_name')
            state = request('text', '--text', 'created-by-agent', '--snapshot', state['snapshot_id'])
            assert state['inputs']['new_folder_name'] == 'created-by-agent'
            request('key', '--key', 'Enter')
            state = request('wait')
            assert state['error'] is None
            assert (fixture / 'created-by-agent').is_dir()
            capture('04-created-folder')
            request('action', 'open_folder', '--name', 'folder')
            state = request('wait')
            assert Path(state['path']) == fixture / 'folder'
            assert state['total_entries'] == 0
            capture('05-empty-folder')
            request('action', 'back')
            request('wait')
            state = click('lens:code')
            assert state['query'].startswith('ext:rs,py')
            assert [entry['name'] for entry in state['entries']] == ['engine.rs']
            capture('06-code-lens')
            state = click('lens:all')
            request('key', '--key', 'Ctrl+P')
            state = request('text', '--text', 'engrs')
            assert state['palette']['open']
            capture('07-fuzzy-jump')
            request('key', '--key', 'Enter')
            state = request('wait')
            assert not state['palette']['open']
            assert state['preview']['open'] and state['preview']['kind'] == 'text'
            assert 'local peek fixture' in state['preview']['text']
            capture('08-local-peek')
            request('key', '--key', 'Ctrl+P')
            request('text', '--text', '>view')
            request('key', '--key', 'ArrowDown')
            state = request('key', '--key', 'Enter')
            assert state['view'] == 'list'
            request('key', '--key', 'Ctrl+P')
            state = request('key', '--key', 'Escape')
            assert not state['palette']['open']
            state = request('key', '--key', 'Space')
            assert not state['preview']['open']
            state = request('action', 'search', '--query', 'kind:file ext:rs -test size:<1kb')
            assert [entry['name'] for entry in state['entries']] == ['engine.rs']
            state = request('action', 'search', '--query', 'size:banana')
            assert state['query_error'] and not state['entries']
            click('clear_filters')
            request('key', '--key', 'Ctrl+F')
            state = request('text', '--text', 'alpha')
            assert state['query'] == 'alpha'

            manifest = {'passed': True, 'session': session, 'checks': [
                'actual renderer PNG capture', 'pixel row selection', 'pixel click and text filtering',
                'keyboard select-all and backspace', 'pixel grid toggle',
                'dialog text/Enter and background folder creation', 'navigation and empty state',
                'composable lenses and invalid query feedback', 'fuzzy file jump and command dispatch',
                'bounded asynchronous local text preview', 'palette Escape handling'],
                'artifacts': sorted(path.name for path in args.output_dir.iterdir())}
            (args.output_dir / 'result.json').write_text(json.dumps(manifest, indent=2))
            print(json.dumps(manifest, indent=2))
        except Exception as error:
            if session is not None:
                try:
                    capture('failure')
                except Exception:
                    pass
            (args.output_dir / 'result.json').write_text(json.dumps({'passed': False, 'session': session, 'error': str(error)}, indent=2))
            raise
        finally:
            if session is not None:
                try:
                    request('wait')
                    request('close')
                except (subprocess.SubprocessError, ValueError, RuntimeError):
                    pass


if __name__ == '__main__':
    main()
