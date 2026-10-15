#!/usr/bin/env python3
"""Managed Unix source installations. Never mutate a contributor's checkout."""
import argparse
import contextlib
import fcntl
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile
import time

UPSTREAM = 'https://github.com/seridian-dev/fyle.git'
BRANCH = 'main'
INTERVAL = 6 * 60 * 60


def read_json(path):
    try:
        return json.loads(path.read_text())
    except FileNotFoundError:
        return {}


def atomic_write(path, data, mode=0o644):
    path.parent.mkdir(parents=True, exist_ok=True)
    fd, temporary = tempfile.mkstemp(prefix='.' + path.name, dir=path.parent)
    try:
        with os.fdopen(fd, 'wb') as output:
            output.write(data)
            output.flush()
            os.fsync(output.fileno())
        os.chmod(temporary, mode)
        os.replace(temporary, path)
    finally:
        Path(temporary).unlink(missing_ok=True)


def write_json(path, value):
    atomic_write(path, (json.dumps(value, indent=2) + '\n').encode())


def pointer(root, name, target):
    temporary = root / ('.' + name + '-' + str(os.getpid()))
    temporary.unlink(missing_ok=True)
    temporary.symlink_to(target, target_is_directory=True)
    try:
        os.replace(temporary, root / name)
    finally:
        temporary.unlink(missing_ok=True)


def current_revision(root):
    current = root / 'current'
    return current.resolve().name if current.is_symlink() else None


def activate(root, binaries, revision):
    if not re.fullmatch(r'[a-zA-Z0-9_-]{1,80}', revision):
        raise ValueError('Invalid revision identifier')
    versions = root / 'versions'
    versions.mkdir(parents=True, exist_ok=True)
    version = versions / revision
    if not version.exists():
        staging = Path(tempfile.mkdtemp(prefix='.staging-', dir=versions))
        try:
            for name in ['fyle', 'fyle-cli']:
                source = binaries / name
                if not source.is_file() or source.stat().st_size == 0 or not os.access(source, os.X_OK):
                    raise RuntimeError(f'Missing or non-executable build output: {source}')
                shutil.copy2(source, staging / name)
            os.replace(staging, version)
        finally:
            if staging.exists():
                shutil.rmtree(staging)
    for name in ['fyle', 'fyle-cli']:
        if not os.access(version / name, os.X_OK):
            raise RuntimeError(f'Incomplete installed version: {version}')
    old = current_revision(root)
    if old and old != revision:
        pointer(root, 'previous', Path('versions') / old)
    # One pointer switches both executables, without modifying a running binary.
    pointer(root, 'current', Path('versions') / revision)


def prune_versions(root):
    keep = {current_revision(root)}
    previous = root / 'previous'
    if previous.is_symlink():
        keep.add(previous.resolve().name)
    versions = root / 'versions'
    candidates = sorted((p for p in versions.iterdir() if p.is_dir() and not p.is_symlink()
                         and re.fullmatch(r'[a-zA-Z0-9_-]{1,80}', p.name)),
                        key=lambda p: p.stat().st_mtime, reverse=True)
    keep.update(p.name for p in candidates[:3])
    for version in candidates:
        if version.name not in keep:
            try:
                shutil.rmtree(version)
            except OSError:
                pass


def command(arguments, **kwargs):
    return subprocess.run(arguments, check=True, text=True, **kwargs)


def git(source, *arguments):
    return command(['git', '-C', str(source), *arguments], capture_output=True, timeout=90).stdout.strip()


def fetch_revision(root):
    source = root / 'source'
    if not source.exists():
        staging = Path(tempfile.mkdtemp(prefix='.source-', dir=root))
        try:
            command(['git', 'clone', '--filter=blob:none', UPSTREAM, str(staging)], timeout=180)
            os.replace(staging, source)
        finally:
            if staging.exists():
                shutil.rmtree(staging)
    if git(source, 'remote', 'get-url', 'origin') != UPSTREAM:
        raise RuntimeError('Updater checkout does not point to the canonical upstream')
    if git(source, 'status', '--porcelain'):
        raise RuntimeError(f'Updater checkout has local changes; refusing to overwrite: {source}')
    git(source, 'fetch', '--no-tags', 'origin', 'refs/heads/' + BRANCH)
    revision = git(source, 'rev-parse', '--verify', 'FETCH_HEAD^{commit}')
    if not re.fullmatch(r'[0-9a-f]{40,64}', revision):
        raise RuntimeError('Upstream returned an invalid commit')
    return revision


def build_revision(root, revision):
    source = root / 'source'
    git(source, 'checkout', '--detach', revision)
    environment = dict(os.environ, CARGO_TARGET_DIR=str(root / 'build'))
    environment.setdefault('CARGO_BUILD_JOBS', str(min(os.cpu_count() or 1, 4)))
    cargo = 'cargo' if shutil.which('cargo') else str(Path.home() / '.cargo/bin/cargo')
    command([cargo, 'build', '--release', '--locked', '--bins'], cwd=source, env=environment, timeout=3600)
    binaries = root / 'build/release'
    for name in ['fyle', 'fyle-cli']:
        # These smoke checks do not require a graphical session.
        args = ['--version'] if name == 'fyle' else ['doctor']
        command([str(binaries / name), *args], capture_output=True, timeout=20)
    return binaries


@contextlib.contextmanager
def install_lock(root):
    root.mkdir(parents=True, exist_ok=True)
    with (root / 'update.lock').open('a') as lock:
        try:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            yield False
        else:
            try:
                yield True
            finally:
                fcntl.flock(lock, fcntl.LOCK_UN)


def update(root, automatic=False, check_only=False):
    config = read_json(root / 'config.json')
    if automatic and not config.get('automatic', False):
        return 'Automatic updates are disabled'
    with install_lock(root) as locked:
        if not locked:
            return 'An update is already running'
        status = read_json(root / 'status.json')
        now = time.time()
        if automatic and now - status.get('attempted_at', 0) < INTERVAL:
            return 'Update check is not due'
        status = dict(status, attempted_at=now, phase='checking', error=None)
        write_json(root / 'status.json', status)
        try:
            revision = fetch_revision(root)
            status['available_revision'] = revision
            if revision == current_revision(root):
                message = 'Fyle is up to date'
            elif check_only:
                message = 'Update available: ' + revision[:12]
                # A manual check must not delay the next automatic install.
                status['attempted_at'] = 0
            else:
                status['phase'] = 'building'
                write_json(root / 'status.json', status)
                binaries = build_revision(root, revision)
                manager = root / 'source/scripts/updater.py'
                manager_bytes = manager.read_bytes()
                compile(manager_bytes, str(manager), 'exec')
                activate(root, binaries, revision)
                atomic_write(root / 'updater.py', manager_bytes, 0o755)
                prune_versions(root)
                message = 'Updated to ' + revision[:12] + '; active on next launch'
            status.update(phase='idle', message=message, installed_revision=current_revision(root), checked_at=now)
            write_json(root / 'status.json', status)
            return message
        except Exception as error:
            detail = getattr(error, 'stderr', None) or str(error)
            status.update(phase='failed', error=detail)
            write_json(root / 'status.json', status)
            raise


def launcher_text(manager, name):
    return ('#!/usr/bin/env python3\nimport os, sys\n'
            f'os.execv(sys.executable, [sys.executable, {str(manager)!r}, "run", {name!r}, *sys.argv[1:]])\n')


def desktop_text(template, executable):
    value = str(executable)
    if '\n' in value or '\r' in value:
        raise ValueError('Install path must not contain newlines')
    for character in ['\\', '"', '`', '$']:
        value = value.replace(character, '\\' + character)
    value = value.replace('\\', '\\\\').replace('%', '%%')
    return template.replace('Exec=fyle', 'Exec="' + value + '"')


def install(root, source, binary_dir, desktop_dir, system=False):
    with install_lock(root) as locked:
        if not locked:
            raise RuntimeError('An update is already running; retry installation after it finishes')
        binaries = Path(os.environ.get('CARGO_TARGET_DIR', str(source / 'target'))) / 'release'
        try:
            revision = git(source, 'rev-parse', 'HEAD')
            if git(source, 'status', '--porcelain'):
                digest = hashlib.sha256(b''.join((binaries / name).read_bytes() for name in ['fyle', 'fyle-cli'])).hexdigest()[:12]
                revision += '-local-' + digest
        except (subprocess.SubprocessError, FileNotFoundError):
            revision = 'local-' + hashlib.sha256((binaries / 'fyle').read_bytes()).hexdigest()[:16]
        activate(root, binaries, revision)
        atomic_write(root / 'updater.py', Path(__file__).read_bytes(), 0o755)
        old_config = read_json(root / 'config.json')
        config = dict(old_config, automatic=old_config.get('automatic', not system), branch=BRANCH, upstream=UPSTREAM)
        if system:
            config['automatic'] = False
        write_json(root / 'config.json', config)
        write_json(root / 'status.json', {'installed_revision': revision, 'phase': 'idle', 'attempted_at': time.time(), 'message': 'Installed'})
        for name in ['fyle', 'fyle-cli']:
            atomic_write(binary_dir / name, launcher_text(root / 'updater.py', name).encode(), 0o755)
        atomic_write(desktop_dir / 'fyle.desktop', desktop_text((source / 'fyle.desktop').read_text(), binary_dir / 'fyle').encode())
        print(f'Installed Fyle ({revision[:12]}) to {binary_dir}; upstream {BRANCH}; automatic updates: {config["automatic"]}')


def maybe_background_update(root):
    if os.environ.get('FYLE_NO_UPDATE') == '1':
        return
    try:
        if not read_json(root / 'config.json').get('automatic', False):
            return
        status = read_json(root / 'status.json')
        if time.time() - status.get('attempted_at', 0) < INTERVAL:
            return
        log = root / 'update.log'
        if log.exists() and log.stat().st_size > 1024 * 1024:
            os.replace(log, root / 'update.previous.log')
        with log.open('ab') as output:
            subprocess.Popen([sys.executable, str(root / 'updater.py'), 'update', '--automatic'],
                             stdin=subprocess.DEVNULL, stdout=output, stderr=output,
                             start_new_session=True, close_fds=True)
    except (OSError, ValueError):
        # Offline/unwritable updater state must never prevent file browsing.
        pass


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path)
    sub = parser.add_subparsers(dest='action', required=True)
    setup = sub.add_parser('install')
    setup.add_argument('--source', type=Path, required=True)
    setup.add_argument('--bin-dir', type=Path, required=True)
    setup.add_argument('--desktop-dir', type=Path, required=True)
    setup.add_argument('--system', action='store_true')
    run = sub.add_parser('run')
    run.add_argument('binary', choices=['fyle', 'fyle-cli'])
    run.add_argument('arguments', nargs=argparse.REMAINDER)
    updater = sub.add_parser('update')
    options = updater.add_mutually_exclusive_group()
    for option in ['check', 'status', 'enable-auto', 'disable-auto', 'rollback', 'automatic']:
        options.add_argument('--' + option, action='store_true')
    args = parser.parse_args()
    if args.action == 'install':
        default_root = Path('/usr/local/lib/fyle') if args.system else Path(os.environ.get('XDG_DATA_HOME', str(Path.home() / '.local/share'))) / 'fyle'
        install((args.root or default_root).resolve(), args.source.resolve(), args.bin_dir.resolve(), args.desktop_dir.resolve(), args.system)
        return
    root = (args.root or Path(__file__).resolve().parent).resolve()
    if args.action == 'run':
        if args.binary == 'fyle-cli' and args.arguments[:1] == ['update']:
            os.execv(sys.executable, [sys.executable, str(root / 'updater.py'), '--root', str(root), 'update', *args.arguments[1:]])
        if args.binary == 'fyle' and not any(arg in ['--version', '-v', '--help', '-h'] for arg in args.arguments):
            maybe_background_update(root)
        executable = root / 'current' / args.binary
        os.execv(str(executable), [str(executable), *args.arguments])
    elif args.status:
        print(json.dumps(dict(read_json(root / 'config.json'), **read_json(root / 'status.json'), current_revision=current_revision(root)), indent=2))
    elif args.enable_auto or args.disable_auto:
        config = read_json(root / 'config.json')
        config['automatic'] = args.enable_auto
        write_json(root / 'config.json', config)
        print('Automatic updates ' + ('enabled' if args.enable_auto else 'disabled'))
    elif args.rollback:
        with install_lock(root) as locked:
            if not locked:
                raise RuntimeError('An update is already running')
            previous = root / 'previous'
            if not previous.is_symlink() or not previous.is_dir():
                raise RuntimeError('No previous version is available')
            activate(root, previous.resolve(), previous.resolve().name)
            config = read_json(root / 'config.json')
            config['automatic'] = False
            write_json(root / 'config.json', config)
            write_json(root / 'status.json', {'phase': 'idle', 'installed_revision': current_revision(root), 'attempted_at': time.time(), 'message': 'Rolled back'})
            print('Rolled back; automatic updates disabled until explicitly enabled')
    else:
        if args.automatic:
            os.nice(10)
        print(update(root, args.automatic, args.check))


if __name__ == '__main__':
    try:
        main()
    except Exception as error:
        print(f'Fyle updater: {error}', file=sys.stderr)
        sys.exit(1)
