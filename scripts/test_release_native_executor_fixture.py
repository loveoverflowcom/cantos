"""Unit rendering checks; systemd and real executor operation remain NOT_RUN."""
import unittest

import release_native as native
import release_native_executor_fixture as fixture
import release_native_forward as forward


class Fixture(unittest.TestCase):
    def test_rendered_units_bind_exact_images_and_existing_unit_parsers(self):
        for service in fixture.SERVICES:
            first = 'ghcr.io/loveoverflowcom/'+service+'@sha256:'+'a'*64
            second = 'ghcr.io/loveoverflowcom/'+service+'@sha256:'+'b'*64
            item = fixture.render(service, first, 'cantos-rehearsal-'+'0'*32)
            self.assertEqual(native.environment_files(item['unit'], root='/root/cantos-native/'),
                             [item['environment_path']])
            changed = forward.migration_unit(item['unit'], first, second)
            self.assertEqual(changed.count(second.encode()), 2)
            self.assertNotIn(first.encode(), changed)
            self.assertIn(b'Pull=never\n', item['unit'])

    def test_foreign_image_and_non_fixture_network_refuse(self):
        for service in fixture.SERVICES:
            valid = 'ghcr.io/loveoverflowcom/'+service+'@sha256:'+'a'*64
            for image in ('ghcr.io/other/'+service+'@sha256:'+'a'*64, valid[:-1]):
                with self.subTest(image=image), self.assertRaises(ValueError):
                    fixture.render(service, image, 'cantos-rehearsal-'+'0'*32)
            with self.assertRaises(ValueError):
                fixture.render(service, valid, 'bridge')


if __name__ == '__main__':
    unittest.main()
