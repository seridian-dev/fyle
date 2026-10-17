#!/usr/bin/env python3
"""Collect reproducible Fyle GUI evidence from the currently logged-in desktop.

This intentionally tests only the active session. It never claims that a
different desktop or init system was exercised just because its packages exist.
"""
import argparse
from datetime import datetime, timezone
import json
import os
from pathlib import Path
import platform
import shlex
import subprocess
import tempfile
import time


def run(args, *, env=None, timeout=30, check=True):
    result = subprocess.run(args, capture_output=True, text=True, env=env, timeout=timeout)
    if check and result.returncode:
        raise RuntimeError(f"{args!r} exited {result.returncode}: {result.stderr.strip()}")
    return result


def distro_info():
    values = {}
    try:
        for line in Path('/etc/os-release').read_text().splitlines():
            key, sep, value = line.partition('=')
            if sep:
                parsed = shlex.split(value, comments=False)
                values[key] = parsed[0] if parsed else ''
    except OSError:
        pass
    return {key: values.get(key) for key in ('ID', 'PRETTY_NAME', 'VERSION_ID')}


def scale_info(doctor):
    info = {key: os.environ.get(key) for key in
            ('GDK_SCALE', 'GDK_DPI_SCALE', 'QT_SCALE_FACTOR', 'QT_AUTO_SCREEN_SCALE_FACTOR', 'XFT_DPI', 'XCURSOR_SIZE')
            if os.environ.get(key) is not None}
    desktop = doctor.get('desktop_environment', '').lower()
    command = None
    if desktop == 'hyprland' and Path('/usr/bin/hyprctl').exists():
        command = ['/usr/bin/hyprctl', 'monitors', '-j']
    elif desktop == 'kde' and Path('/usr/bin/kscreen-doctor').exists():
        command = ['/usr/bin/kscreen-doctor', '-o']
    elif desktop == 'gnome' and Path('/usr/bin/gsettings').exists():
        command = ['/usr/bin/gsettings', 'get', 'org.gnome.desktop.interface', 'text-scaling-factor']
    if command:
        result = run(command, check=False)
        info['compositor_query'] = {'command': command, 'exit_code': result.returncode}
        if result.returncode == 0 and desktop == 'hyprland':
            try:
                info['monitors'] = [{key: monitor.get(key) for key in
                                     ('name', 'width', 'height', 'refreshRate', 'scale', 'transform', 'focused')}
                                    for monitor in json.loads(result.stdout)]
            except (ValueError, TypeError):
                pass
        elif result.stdout.strip():
            info['desktop_scale_setting'] = result.stdout.strip()[:512]
    return info


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--cli', type=Path, default=Path('target/debug/fyle-cli'))
    parser.add_argument('--output-dir', type=Path, required=True)
    args = parser.parse_args()
    args.output_dir.mkdir(parents=True, exist_ok=True)
    if any(args.output_dir.iterdir()):
        raise SystemExit('Use a new or empty evidence directory')
    cli = str(args.cli.resolve())
    env = os.environ.copy()
    checks = []
    session = None
    report = {
        'schema_version': 1,
        'recorded_at': datetime.now(timezone.utc).isoformat(),
        'host': {'distro': distro_info(), 'kernel': platform.release(),
                 'architecture': platform.machine(), 'desktop': {}, 'scale': {}},
        'checks': checks,
        'limitations': ['Only the active logged-in desktop/init combination was exercised.'],
    }

    def cli_run(*argv):
        result = run([cli, 'gui', *map(str, argv)], env=env)
        return json.loads(result.stdout)

    def request(*argv):
        return cli_run(*argv, '--session', session)

    def click_control(control_id):
        for attempt in range(5):
            state = request('state')
            control = next(control for control in state['controls'] if control['id'] == control_id)
            x, y, width, height = control['rect_pixels']
            try:
                return request('click', '--x', x + width / 2, '--y', y + height / 2,
                               '--snapshot', state['snapshot_id'])
            except RuntimeError as error:
                if 'Stale snapshot' not in str(error) or attempt == 4:
                    raise
                time.sleep(0.1)

    def record(name, detail):
        checks.append({'name': name, 'result': 'pass', 'evidence': detail})

    try:
        doctor = json.loads(run([cli, 'doctor', '--json'], env=env).stdout)
        report['host']['desktop'] = {key: doctor.get(key) for key in
            ('desktop', 'desktop_environment', 'session_desktop', 'session_type', 'display_backend',
             'display_present', 'wayland_present', 'dbus_session_present', 'init_system', 'terminal',
             'xdg_open_available')}
        report['host']['scale'] = scale_info(doctor)
        record('doctor-session-detection', report['host']['desktop'])
        record('native-scaling-probe', report['host']['scale'])

        with tempfile.TemporaryDirectory(prefix='fyle-native-') as temporary:
            root = Path(temporary)
            folder = root / 'space and 漢字'
            folder.mkdir()
            (folder / 'hello.txt').write_text('native desktop fixture\n')
            (folder / 'code.rs').write_text('fn main() {}\n')
            env['FYLE_SETTINGS_DIR'] = str(root / 'settings')
            launched = cli_run('launch', '--path', root)
            session = launched['session']
            assert launched['state']['ready'], launched['state']
            state = request('action', 'navigate', '--path', folder.as_uri())
            assert Path(state['path']) == folder, state['path']
            record('file-url-with-spaces-and-unicode', {'uri': folder.as_uri(), 'resolved_path': state['path']})
            screenshot = request('screenshot', '--output', args.output_dir / 'native-session.png')
            assert screenshot['width'] > 0 and screenshot['height'] > 0
            record('native-renderer-screenshot', {'width': screenshot['width'], 'height': screenshot['height'],
                                                   'backend': doctor.get('display_backend'),
                                                   'shader_available': screenshot['state']['appearance']['shader_available'],
                                                   'software_gl_forced': env.get('LIBGL_ALWAYS_SOFTWARE') == '1'})

            mime = {'file_type': run(['xdg-mime', 'query', 'filetype', str(folder / 'hello.txt')], check=False).stdout.strip(),
                    'default_text_application': run(['xdg-mime', 'query', 'default', 'text/plain'], check=False).stdout.strip()}
            state = request('action', 'edit_file', '--name', 'hello.txt')
            state = request('wait')
            assert state['editor']['open'] and Path(state['editor']['items'][0]['path']) == folder / 'hello.txt'
            record('text-file-routing-and-host-mime-map', {'fyle_route': 'native editor', 'host_mime': mime})
            request('action', 'editor_close')

            destination = folder / 'paste destination'
            destination.mkdir()
            state = request('action', 'select', '--name', 'hello.txt')
            assert state['selected_count'] == 1
            request('key', '--key', 'Ctrl+C')
            request('action', 'navigate', '--path', destination)
            request('wait')
            request('key', '--key', 'Ctrl+V')
            state = request('state')
            assert state['dialog'] == 'confirm_copy', state['dialog']
            click_control('copy_confirm')
            request('wait')
            assert (destination / 'hello.txt').read_text() == 'native desktop fixture\n'
            record('fyle-internal-clipboard-copy-paste', {'copied': str(destination / 'hello.txt')})

            state = request('action', 'terminal_open')
            assert state['terminal']['running'], state['terminal']
            request('action', 'terminal_send', '--command', 'printf FYLE_NATIVE_PTY_OK')
            deadline = time.monotonic() + 5
            while time.monotonic() < deadline:
                state = request('state')
                if 'FYLE_NATIVE_PTY_OK' in state['terminal']['screen']:
                    break
                time.sleep(0.05)
            assert 'FYLE_NATIVE_PTY_OK' in state['terminal']['screen'], state['terminal']
            record('embedded-pty-on-native-session', {'pid': state['terminal']['pid'],
                                                        'dimensions': [state['terminal']['rows'], state['terminal']['cols']]})
            state = request('action', 'terminal_close')
            assert not state['terminal']['running']
            record('pty-session-cleanup', {'running_after_close': state['terminal']['running']})
            request('close')
            session = None
    except Exception as error:
        checks.append({'name': 'native-session-run', 'result': 'fail', 'evidence': str(error)})
        report['passed'] = False
        raise
    finally:
        if session:
            try:
                cli_run('action', 'terminal_close', '--session', session)
                cli_run('close', '--session', session)
            except Exception as error:
                checks.append({'name': 'cleanup', 'result': 'fail', 'evidence': str(error)})
        report['passed'] = all(check['result'] == 'pass' for check in checks)
        (args.output_dir / 'native-desktop-report.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(report, indent=2))


if __name__ == '__main__':
    main()
