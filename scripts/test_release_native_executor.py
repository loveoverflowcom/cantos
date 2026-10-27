"""Executor provenance/portability refusals; actual static build is qualified on Linux CI."""
from pathlib import Path
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

import release_native_executor as executor


class Executor(unittest.TestCase):
    def test_source_and_compiler_are_pinned_without_a_deployment_toolchain_override(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root/'rust-toolchain.toml').write_text('[toolchain]\nchannel="1.98.1"\n')
            linux = SimpleNamespace(system=lambda: 'Linux', machine=lambda: 'x86_64')
            with patch.object(executor.oci, 'git', side_effect=[b'a'*40+b'\n', b'']):
                sha, pin, checkout = executor.identity(root, 'rustc 1.98.1 (fixture)', system=linux)
            self.assertEqual((sha, pin, str(checkout)), ('a'*40, '1.98.1', '/opt/cantos-native-executor/'+'a'*40))
            with patch.object(executor.oci, 'git', side_effect=[b'a'*40, b'']):
                with self.assertRaisesRegex(ValueError, 'exact pinned'):
                    executor.identity(root, 'rustc 1.97.0 (fixture)', system=linux)
            with patch.object(executor.oci, 'git', side_effect=[b'a'*40, b' M changed']):
                with self.assertRaisesRegex(ValueError, 'clean executor source'):
                    executor.identity(root, 'rustc 1.98.1 (fixture)', system=linux)
            with self.assertRaisesRegex(ValueError, 'native Linux amd64'):
                executor.identity(root, 'rustc 1.98.1 (fixture)', system=SimpleNamespace(system=lambda: 'Darwin'))

    def test_dynamic_loader_shared_libraries_wrong_architecture_and_non_elf_refuse(self):
        with tempfile.TemporaryDirectory() as temporary:
            binary = Path(temporary)/'xtask'
            data = bytearray(64); data[:6] = b'\x7fELF\x02\x01'; data[18:20] = b'\x3e\x00'
            binary.write_bytes(data)
            self.assertEqual(executor.static_binary(binary, 'LOAD', 'no dynamic section')['bytes'], 64)
            for headers, dynamic in [('INTERP', ''), ('LOAD', '(NEEDED) libc.so')]:
                with self.assertRaisesRegex(ValueError, 'static amd64 ELF'):
                    executor.static_binary(binary, headers, dynamic)
            for offset, value in [(0, 0), (4, 1), (18, 183)]:
                changed = data.copy(); changed[offset] = value; binary.write_bytes(changed)
                with self.assertRaisesRegex(ValueError, 'static amd64 ELF'):
                    executor.static_binary(binary, 'LOAD', '')


if __name__ == '__main__':
    unittest.main()
