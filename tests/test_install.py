"""Installer contract tests without compiling Rust or touching user integration."""
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]


class InstallerTests(unittest.TestCase):
    def test_xdg_install_special_paths_and_uninstall(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            source = root / 'source'
            source.mkdir()
            for name in ['install.sh', 'fyle.desktop']:
                shutil.copy2(ROOT / name, source / name)
            shutil.copytree(ROOT / 'scripts', source / 'scripts')
            tools = root / 'tools'
            tools.mkdir()
            cargo = tools / 'cargo'
            cargo.write_text('#!/bin/sh\nmkdir -p target/release\nprintf "#!/bin/sh\\nexit 0\\n" > target/release/fyle\ncp target/release/fyle target/release/fyle-cli\nchmod +x target/release/fyle target/release/fyle-cli\n')
            cargo.chmod(0o755)
            binary_dir = root / 'bin space $value %test'
            data = root / 'xdg data'
            env = dict(os.environ, PATH=f'{tools}:{os.environ["PATH"]}',
                       INSTALL_DIR=str(binary_dir), XDG_DATA_HOME=str(data))
            env.pop('DESKTOP_DIR', None)
            subprocess.run(['bash', str(source / 'install.sh')], env=env, check=True)
            entry = data / 'applications/fyle.desktop'
            text = entry.read_text()
            self.assertIn('--downloads', text)
            self.assertNotIn('~', text)
            self.assertIn('%%test', text)
            self.assertTrue((binary_dir / 'fyle-cli').is_file())
            subprocess.run([str(binary_dir / 'fyle'), '--version'], check=True)
            subprocess.run([str(binary_dir / 'fyle-cli'), 'update', '--status'], check=True, capture_output=True)
            managed = data / 'fyle'
            self.assertTrue((managed / 'current/fyle').is_file())
            subprocess.run([str(binary_dir / 'fyle-cli'), 'update', '--disable-auto'], check=True, capture_output=True)
            import json
            self.assertFalse(json.loads((managed / 'config.json').read_text())['automatic'])
            if shutil.which('desktop-file-validate'):
                subprocess.run(['desktop-file-validate', str(entry)], check=True)
            subprocess.run(['bash', str(source / 'install.sh'), '--uninstall'], env=env, check=True)
            self.assertFalse(entry.exists())
            self.assertFalse((binary_dir / 'fyle').exists())
            self.assertFalse((binary_dir / 'fyle-cli').exists())

    def test_unknown_options_fail(self):
        result = subprocess.run(['bash', str(ROOT / 'install.sh'), '--typo'], capture_output=True)
        self.assertEqual(result.returncode, 2)


if __name__ == '__main__':
    unittest.main()
