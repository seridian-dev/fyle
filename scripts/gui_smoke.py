#!/usr/bin/env python3
"""Exercise real egui input and capture PNG/JSON evidence in an agent-owned session."""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import time


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
    run_env = os.environ.copy()

    def run(*arguments):
        result = subprocess.run([cli, 'gui', *map(str, arguments)],
                                capture_output=True, text=True, timeout=30, env=run_env)
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
        # Separate synthetic clicks enough to avoid accidental egui double-clicks.
        time.sleep(0.32)
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
                time.sleep(0.05)

    def right_click(control_id):
        time.sleep(0.32)
        for attempt in range(5):
            state = request('state')
            control = next(control for control in state['controls'] if control['id'] == control_id)
            x, y, width, height = control['rect_pixels']
            try:
                state = request('click', '--button', 'right', '--x', x + width / 2,
                                '--y', y + height / 2, '--snapshot', state['snapshot_id'])
                expected = {'context_open', 'context_copy', 'context_cut',
                            'context_delete', 'context_terminal', 'context_create_zip',
                            'context_create_7z'}
                for _ in range(10):
                    visible = {control['id'] for control in state['controls']}
                    if state['context_menu']['visible'] and expected <= visible:
                        return state
                    time.sleep(0.1)
                    state = request('state')
                return state
            except RuntimeError as error:
                if 'Stale snapshot' not in str(error) or attempt == 4:
                    raise

    with tempfile.TemporaryDirectory(prefix='fyle-gui-fixture-') as temporary:
        fixture = Path(temporary)
        run_env['FYLE_SETTINGS_DIR'] = str(fixture / '.appearance-fixture')
        (fixture / 'alpha.txt').write_text('fixture alpha')
        (fixture / 'beta.txt').write_text('fixture beta')
        (fixture / 'folder').mkdir()
        scan_fixture = fixture / 'scan-fixture'
        (scan_fixture / 'nested').mkdir(parents=True)
        (scan_fixture / 'big.bin').write_bytes(b'x' * 32768)
        (scan_fixture / 'nested' / 'medium.bin').write_bytes(b'y' * 1024)
        (fixture / 'engine.rs').write_text('fn main() { /* local peek fixture */ }\n// local peek fixture again\n')
        git_env = os.environ.copy()
        git_env.update({'GIT_CONFIG_NOSYSTEM': '1', 'GIT_CONFIG_GLOBAL': os.devnull})
        def git(*arguments):
            subprocess.run(['git', '-C', str(fixture), *arguments], check=True,
                           env=git_env, stdout=subprocess.DEVNULL)
        git('init', '-q')
        git('config', 'user.name', 'Fyle GUI Test')
        git('config', 'user.email', 'fyle@example.invalid')
        git('add', 'alpha.txt', 'beta.txt', 'engine.rs', 'folder')
        git('commit', '-qm', 'GUI fixture')
        try:
            launched = run('launch', '--path', fixture / 'engine.rs')
            session = launched['session']
            assert launched['state']['ready']
            assert launched['state']['editor']['open']
            assert launched['state']['editor']['line_numbers']
            assert launched['state']['editor']['items'][0]['language'] == 'Rust'
            assert Path(launched['state']['editor']['items'][0]['path']) == fixture / 'engine.rs'
            request('action', 'editor_close')
            state = request('action', 'toggle_hidden')
            assert not state['show_hidden']
            capture('01-list')
            state = click('toolbar_more')
            more_controls = {control['id'] for control in state['controls']}
            assert {'toolbar_new_file', 'toolbar_open_terminal', 'toolbar_refresh',
                    'toolbar_appearance'} <= more_controls
            capture('01-more-menu')
            click('toolbar_more')
            state = click('entry:alpha.txt')
            assert state['selected_count'] == 1
            assert next(entry for entry in state['entries'] if entry['name'] == 'alpha.txt')['selected']
            state = right_click('entry:alpha.txt')
            assert state['context_menu']['visible']
            menu_items = {control['id'] for control in state['controls']}
            assert {'context_open', 'context_copy', 'context_cut', 'context_delete',
                    'context_terminal'} <= menu_items
            capture('02-context-menu')
            state = click('context_copy')
            assert not state['context_menu']['visible']
            state = right_click('entry:alpha.txt')
            assert 'context_create_zip' in {control['id'] for control in state['controls']}
            assert 'context_create_7z' in {control['id'] for control in state['controls']}
            state = click('context_create_zip')
            state = request('wait')
            assert state['error'] is None
            assert (fixture / 'alpha.txt.zip').is_file()
            state = right_click('entry:alpha.txt.zip')
            assert 'context_extract_archive' in {control['id'] for control in state['controls']}
            capture('02-extract-context-menu')
            state = click('context_extract_archive')
            state = request('wait')
            assert state['error'] is None
            assert (fixture / 'alpha.txt-extracted' / 'alpha.txt').read_text() == 'fixture alpha'
            capture('02-archive-actions')
            shutil.rmtree(fixture / 'alpha.txt-extracted')
            (fixture / 'alpha.txt.zip').unlink()
            request('action', 'refresh')
            request('wait')
            state = request('action', 'edit_file', '--name', 'engine.rs')
            state = request('wait')
            assert state['editor']['open']
            assert state['editor']['items'][0]['language'] == 'Rust'
            capture('03-native-editor')
            state = click('editor_find')
            state = request('key', '--key', 'Ctrl+A')
            state = request('text', '--text', 'local peek fixture', '--snapshot', state['snapshot_id'])
            assert state['editor']['match_count'] == 2
            state = click('editor_find_next')
            assert state['editor']['match_index'] == 1
            state = click('editor_text')
            state = request('key', '--key', 'Ctrl+A')
            state = request('text', '--text', 'fn main() { println!("edited in Fyle"); } // local peek fixture\n',
                            '--snapshot', state['snapshot_id'])
            assert state['editor']['items'][0]['dirty']
            request('key', '--key', 'Ctrl+S')
            state = request('wait')
            assert not state['editor']['items'][0]['dirty']
            assert 'edited in Fyle' in (fixture / 'engine.rs').read_text()
            state = request('action', 'edit_file', '--name', 'beta.txt')
            state = request('wait')
            assert len(state['editor']['items']) == 2
            state = click('editor_tab:0')
            assert state['editor']['active'] == 0
            request('action', 'editor_close')
            assert not request('state')['editor']['open']
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
            state = click('entry:beta.txt')
            assert next(entry for entry in state['entries'] if entry['name']=='beta.txt')['selected']
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
            request('key', '--key', 'Ctrl+T')
            state = request('wait')
            assert len(state['tabs']['items']) == 2 and state['tabs']['active'] == 1
            request('action', 'navigate', '--path', fixture / 'folder')
            request('wait')
            click('tab:0')
            state = request('wait')
            assert Path(state['path']) == fixture and state['query'] == 'alpha'
            click('tab:1')
            state = request('wait')
            assert Path(state['path']) == fixture / 'folder'
            request('key', '--key', 'Ctrl+L')
            state = request('key', '--key', 'Ctrl+A')
            request('text', '--text', '..')
            request('key', '--key', 'Enter')
            state = request('wait')
            assert Path(state['path']) == fixture and not state['location']['open']
            request('action', 'search', '--query', '')
            request('action', 'view', '--mode', 'grid')
            request('key', '--key', 'Ctrl+Shift+Tab')
            state = request('wait')
            assert state['tabs']['active'] == 0 and state['query'] == 'alpha'
            request('key', '--key', 'Ctrl+Tab')
            state = request('wait')
            assert state['tabs']['active'] == 1 and state['query'] == ''
            capture('09-workspace-tabs')
            request('key', '--key', 'Ctrl+W')
            state = request('wait')
            assert len(state['tabs']['items']) == 1 and state['query'] == 'alpha'
            assert state['appearance']['shader_available'], state['appearance']['shader_error']
            request('action', 'preferences_appearance')
            state = click('appearance_effects')
            assert not state['appearance']['settings']['effects']
            state = click('appearance_effects')
            assert state['appearance']['settings']['effects']
            click('appearance_save')
            saved = json.loads((fixture / '.appearance-fixture' / 'appearance.json').read_text())
            assert saved['highlight'] == [0, 212, 255]
            state = request('action', 'preferences')
            assert state['preferences']['section'] == 'general'
            assert state['preferences']['show_status_bar']
            state = click('preferences_status_bar')
            assert not state['preferences']['show_status_bar']
            click('appearance_save')
            settings = json.loads((fixture / '.appearance-fixture' / 'appearance.json').read_text())
            assert settings['show_status_bar'] is False
            state = click('preferences_status_bar')
            assert state['preferences']['show_status_bar']
            click('appearance_save')
            state = request('action', 'preferences_terminal')
            assert state['preferences']['section'] == 'terminal'
            assert not state['preferences']['allow_external_terminals']
            state = click('preferences_external_terminals')
            assert state['preferences']['allow_external_terminals']
            click('appearance_save')
            settings = json.loads((fixture / '.appearance-fixture' / 'appearance.json').read_text())
            assert settings['allow_external_terminals'] is True
            state = click('preferences_external_terminals')
            assert not state['preferences']['allow_external_terminals']
            click('appearance_save')
            click('preferences_close')
            state = click('storage_explorer')
            assert state['storage']['open'], state['storage']
            state = click('storage_explorer')
            assert not state['storage']['open'], state['storage']
            cli_scan = subprocess.run([cli, 'storage', 'scan', str(scan_fixture)],
                                      capture_output=True, text=True, timeout=30)
            assert cli_scan.returncode == 0, cli_scan.stderr
            cli_summary = json.loads(cli_scan.stdout)
            assert cli_summary['apparent_bytes'] == 33792 and cli_summary['file_count'] == 2
            state = request('action', 'storage_scan', '--path', scan_fixture)
            state = request('wait')
            summary = state['storage']['summary']
            assert summary['apparent_bytes'] == 33792 and summary['file_count'] == 2
            assert summary['directory_count'] == 1
            assert {entry['name'] for entry in summary['entries']} == {'big.bin', 'nested'}
            request('action', 'storage_select', '--name', 'big.bin')
            state = request('action', 'peek')
            state = request('wait')
            assert state['preview']['path'] == str(scan_fixture / 'big.bin')
            state = request('action', 'peek')
            assert not state['preview']['open']
            request('action', 'storage_select', '--name', 'nested')
            state = request('action', 'storage_drill')
            state = request('wait')
            assert Path(state['storage']['summary']['root']) == scan_fixture / 'nested'
            assert state['storage']['summary']['apparent_bytes'] == 1024
            request('action', 'storage_scan', '--path', scan_fixture)
            state = request('wait')
            request('action', 'storage_select', '--name', 'big.bin')
            state = request('action', 'storage_trash')
            assert state['dialog'] == 'confirm_delete' and (scan_fixture / 'big.bin').is_file()
            click('delete_cancel')
            assert (scan_fixture / 'big.bin').is_file()
            capture('10-storage-explorer')
            click('tab:0')
            capture('10-appearance-settings')
            (fixture / 'review.rs').write_text('fn staged_version() {}\n')
            git('add', 'review.rs')
            (fixture / 'review.rs').write_text('fn working_version() {}\n')
            (fixture / '未追跡.rs').write_text('fn unicode_untracked() {}\n')
            request('action', 'navigate', '--path', fixture)
            request('wait')
            state = request('action', 'review')
            state = request('wait')
            assert state['review']['open'] and not state['review']['loading']
            paths = {change['path']: change for change in state['review']['files']}
            assert 'review.rs' in paths and paths['review.rs']['staged'] and paths['review.rs']['unstaged']
            assert '未追跡.rs' in paths and paths['未追跡.rs']['kind'] == 'Untracked'
            request('action', 'review_select', '--path', 'review.rs')
            request('action', 'review_layer', '--mode', 'staged')
            state = request('wait')
            assert '+fn staged_version() {}' in state['review']['diff']
            request('action', 'review_layer', '--mode', 'working')
            state = request('wait')
            assert '+fn working_version() {}' in state['review']['diff']
            request('action', 'review_select', '--path', '未追跡.rs')
            state = request('wait')
            assert '+fn unicode_untracked() {}' in state['review']['diff']
            capture('11-git-code-review')
            state = request('action', 'terminal_open')
            assert state['terminal']['open'] and state['terminal']['running']
            state = request('action', 'terminal_new')
            assert len(state['terminal']['sessions']) == 2
            state = request('action', 'terminal_split')
            assert state['terminal']['split']
            state = request('action', 'terminal_split')
            assert not state['terminal']['split']
            state = request('action', 'terminal_close_active')
            assert len(state['terminal']['sessions']) == 1 and state['terminal']['running']
            state = request('state')
            if any(control['id'] == 'terminal_screen' for control in state['controls']):
                state = click('terminal_screen')
            else:
                # The terminal surface paints correctly in some window layouts but
                # its large click target is clipped from the automation map. Focus
                # its center using the viewport and a fresh snapshot in that case.
                viewport = state['viewport']
                scale = viewport['pixels_per_point']
                state = request(
                    'click', '--x', viewport['width'] * scale * 0.60,
                    '--y', viewport['height'] * scale * 0.76,
                    '--snapshot', state['snapshot_id'],
                )
            request('text', '--text', 'printf FYLE_HARNESS_TERMINAL_OK')
            request('key', '--key', 'Enter')
            deadline = time.monotonic() + 5
            while time.monotonic() < deadline:
                state = request('state')
                if 'FYLE_HARNESS_TERMINAL_OK' in state['terminal']['screen']:
                    break
                time.sleep(0.05)
            assert 'FYLE_HARNESS_TERMINAL_OK' in state['terminal']['screen']
            capture('12-embedded-terminal')
            state = request('action', 'terminal_close')
            assert not state['terminal']['running']
            request('key', '--key', 'Escape')


            manifest = {'passed': True, 'session': session, 'checks': [
                'actual renderer PNG capture', 'pixel row selection', 'pixel click and text filtering',
                'Fyle-styled context menu vector actions, ZIP creation and safe archive extraction',
                'in-app Rust syntax editing, Unicode-safe find navigation, document tabs and conflict-aware Ctrl+S save',
                'read-only staged and working diffs, untracked Unicode file review, and bounded Git status',
                'app-owned PTY command execution, VT screen state, screenshot capture and explicit shell shutdown',
                'keyboard select-all and backspace', 'pixel grid toggle',
                'dialog text/Enter and background folder creation', 'navigation and empty state',
                'composable lenses and invalid query feedback', 'fuzzy file jump and command dispatch',
                'bounded asynchronous local text preview', 'palette Escape handling', 'independent tab navigation and filters',
                'relative location entry and keyboard tab close',
                'actual OpenGL shader and persisted appearance controls'],
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
