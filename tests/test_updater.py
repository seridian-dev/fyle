"""Updater activation must preserve a usable installed version on failure."""
import importlib.util
from pathlib import Path
import tempfile
import subprocess
import time
import unittest
from unittest.mock import patch

SCRIPT = Path(__file__).resolve().parents[1] / 'scripts/updater.py'
spec = importlib.util.spec_from_file_location('fyle_updater', SCRIPT)
updater = importlib.util.module_from_spec(spec)
spec.loader.exec_module(updater)


class UpdaterTests(unittest.TestCase):
    def test_activation_swaps_both_binaries_as_one_version(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            for revision in ['old', 'new']:
                binaries = root / revision
                binaries.mkdir()
                for name in ['fyle', 'fyle-cli']:
                    executable = binaries / name
                    executable.write_text('#!/bin/sh\nexit 0\n')
                    executable.chmod(0o755)
                updater.activate(root / 'install', binaries, revision)
            self.assertEqual((root / 'install/current').resolve().name, 'new')
            self.assertTrue((root / 'install/versions/old/fyle').is_file())
            self.assertTrue((root / 'install/current/fyle-cli').is_file())

    def test_rollback_switches_version_and_disables_automatic_updates(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            binaries = root / 'binaries'
            binaries.mkdir()
            for name in ['fyle', 'fyle-cli']:
                (binaries / name).write_text('#!/bin/sh\nexit 0\n')
                (binaries / name).chmod(0o755)
            updater.activate(root, binaries, 'old')
            updater.activate(root, binaries, 'new')
            with patch('sys.argv', [str(SCRIPT), '--root', str(root), 'update', '--rollback']):
                updater.main()
            self.assertEqual(updater.current_revision(root), 'old')
            self.assertFalse(updater.read_json(root / 'config.json')['automatic'])

    def test_successful_update_activates_only_complete_build(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            binaries = root / 'binaries'
            binaries.mkdir()
            for name in ['fyle', 'fyle-cli']:
                (binaries / name).write_text('#!/bin/sh\nexit 0\n')
                (binaries / name).chmod(0o755)
            updater.activate(root, binaries, 'old')
            manager = root / 'source/scripts/updater.py'
            manager.parent.mkdir(parents=True)
            manager.write_bytes(SCRIPT.read_bytes())
            with patch.object(updater, 'fetch_revision', return_value='new'), patch.object(updater, 'build_revision', return_value=binaries):
                updater.update(root)
            self.assertEqual(updater.current_revision(root), 'new')
            self.assertEqual((root / 'previous').resolve().name, 'old')
            self.assertEqual(updater.read_json(root / 'status.json')['phase'], 'idle')

    def test_failed_staging_never_changes_current(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            binaries = root / 'binaries'
            binaries.mkdir()
            for name in ['fyle', 'fyle-cli']:
                (binaries / name).write_text('old')
                (binaries / name).chmod(0o755)
            updater.activate(root / 'install', binaries, 'old')
            (binaries / 'fyle-cli').unlink()
            with self.assertRaises(Exception):
                updater.activate(root / 'install', binaries, 'broken')
            self.assertEqual((root / 'install/current').resolve().name, 'old')

    def test_launcher_arguments_are_not_interpreted_by_a_shell(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / 'space $variable %test'
            wrapper = updater.launcher_text(root / 'updater.py', 'fyle')
            compile(wrapper, '<launcher>', 'exec')
            self.assertIn(repr(str(root / 'updater.py')), wrapper)

    def test_failed_build_keeps_current_and_records_failure(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            updater.write_json(root / 'config.json', {'automatic': True})
            with patch.object(updater, 'fetch_revision', return_value='new'), patch.object(updater, 'build_revision', side_effect=RuntimeError('build failed')):
                with self.assertRaisesRegex(RuntimeError, 'build failed'):
                    updater.update(root)
            self.assertIn('build failed', updater.read_json(root / 'status.json')['error'])
            self.assertFalse((root / 'current').exists())

    def test_automatic_checks_are_throttled_and_can_be_disabled(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            updater.write_json(root / 'config.json', {'automatic': False})
            with patch.object(updater, 'fetch_revision') as fetch:
                updater.update(root, automatic=True)
                fetch.assert_not_called()
            updater.write_json(root / 'config.json', {'automatic': True})
            updater.write_json(root / 'status.json', {'attempted_at': time.time()})
            with patch.object(updater, 'fetch_revision') as fetch:
                updater.update(root, automatic=True)
                fetch.assert_not_called()

    def test_concurrent_update_is_nonblocking(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            with updater.install_lock(root) as first:
                self.assertTrue(first)
                with updater.install_lock(root) as second:
                    self.assertFalse(second)

    def test_fetch_uses_dedicated_checkout_and_refuses_local_changes(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            upstream = root / 'upstream'
            upstream.mkdir()
            subprocess.run(['git', 'init', '-b', 'main', str(upstream)], check=True, capture_output=True)
            for key, value in [('user.email', 'tests@example.invalid'), ('user.name', 'Updater tests')]:
                updater.git(upstream, 'config', key, value)
            (upstream / 'README').write_text('first')
            updater.git(upstream, 'add', '.')
            updater.git(upstream, 'commit', '-m', 'first')
            installation = root / 'installation'
            installation.mkdir()
            with patch.object(updater, 'UPSTREAM', str(upstream)):
                first = updater.fetch_revision(installation)
                self.assertEqual(first, updater.git(upstream, 'rev-parse', 'HEAD'))
                (upstream / 'README').write_text('second')
                updater.git(upstream, 'commit', '-am', 'second')
                second = updater.fetch_revision(installation)
                self.assertNotEqual(first, second)
                (installation / 'source/README').write_text('user changes')
                with self.assertRaisesRegex(RuntimeError, 'local changes'):
                    updater.fetch_revision(installation)
                self.assertEqual((installation / 'source/README').read_text(), 'user changes')

    def test_complete_update_builds_in_managed_cache_and_reuses_installed_revision(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            upstream = root / 'upstream'
            (upstream / 'scripts').mkdir(parents=True)
            (upstream / 'scripts/updater.py').write_bytes(SCRIPT.read_bytes())
            subprocess.run(['git', 'init', '-b', 'main', str(upstream)], check=True, capture_output=True)
            for key, value in [('user.email', 'tests@example.invalid'), ('user.name', 'Updater tests')]:
                updater.git(upstream, 'config', key, value)
            updater.git(upstream, 'add', '.')
            updater.git(upstream, 'commit', '-m', 'fixture')
            tools = root / 'tools'
            tools.mkdir()
            cargo = tools / 'cargo'
            cargo.write_text("#!/usr/bin/env python3\nimport os\nfrom pathlib import Path\nout = Path(os.environ['CARGO_TARGET_DIR']) / 'release'\nout.mkdir(parents=True, exist_ok=True)\nfor name in ['fyle', 'fyle-cli']:\n    binary = out / name\n    binary.write_text('#!/bin/sh\\necho fixture\\n')\n    binary.chmod(0o755)\n")
            cargo.chmod(0o755)
            original_command = updater.command
            builds = []
            def run(arguments, **kwargs):
                if arguments[0] == 'cargo':
                    builds.append(arguments)
                    environment = dict(kwargs['env'])
                    environment['PATH'] = str(tools) + ':' + environment['PATH']
                    kwargs['env'] = environment
                return original_command(arguments, **kwargs)
            installation = root / 'installation'
            with patch.object(updater, 'UPSTREAM', str(upstream)), patch.object(updater, 'command', side_effect=run):
                updater.update(installation)
                self.assertIn('up to date', updater.update(installation))
            self.assertEqual(len(builds), 1)
            self.assertEqual(updater.current_revision(installation), updater.git(upstream, 'rev-parse', 'HEAD'))
            self.assertEqual(updater.git(upstream, 'status', '--porcelain'), '')
            result = subprocess.run([str(installation / 'current/fyle'), '--version'], check=True, capture_output=True, text=True)
            self.assertEqual(result.stdout.strip(), 'fixture')

    def test_offline_update_records_failure_without_changing_version(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            (root / 'versions/old').mkdir(parents=True)
            updater.pointer(root, 'current', Path('versions/old'))
            with patch.object(updater, 'fetch_revision', side_effect=OSError('offline')):
                with self.assertRaisesRegex(OSError, 'offline'):
                    updater.update(root)
            self.assertEqual(updater.current_revision(root), 'old')
            self.assertEqual(updater.read_json(root / 'status.json')['phase'], 'failed')


if __name__ == '__main__':
    unittest.main()
