"""Release-note rendering uses only real build assets and explicit commit types."""
import importlib.util
import pathlib
import tempfile
import unittest
from unittest.mock import patch

PATH = pathlib.Path(__file__).resolve().parents[1] / 'scripts' / 'release-notes.py'
spec = importlib.util.spec_from_file_location('release_notes', PATH)
notes = importlib.util.module_from_spec(spec)
spec.loader.exec_module(notes)


class ReleaseNotesTests(unittest.TestCase):
    def test_asset_links_only_include_built_downloads(self):
        with tempfile.TemporaryDirectory() as tmp:
            folder = pathlib.Path(tmp)
            (folder / 'atlas-commerce-desktop-v1.2.3-macos-universal.dmg').touch()
            (folder / 'atlas-commerce-desktop-v1.2.3-source.tar.gz').touch()
            (folder / 'checksums.txt').touch()
            with patch.object(notes, 'previous_tag', return_value='v1.2.2'), patch.object(notes, 'change_groups', return_value={'Corrections': ['Corriger la connexion']}):
                body = notes.render_body('1.2.3', '', 'owner/repo', folder)
            self.assertIn('/releases/download/v1.2.3/atlas-commerce-desktop-v1.2.3-macos-universal.dmg', body)
            self.assertNotIn('source.tar.gz', body)
            self.assertNotIn('checksums.txt', body)
            self.assertIn('**Corrections**', body)
            self.assertIn('/compare/v1.2.2...v1.2.3', body)

    def test_custom_notes_are_preserved(self):
        with patch.object(notes, 'previous_tag', return_value=None):
            body = notes.render_body('1.2.3', 'Résumé\n\n**Nouveau**\n\n- Action', 'owner/repo', None)
        self.assertIn('Résumé\n\n**Nouveau**\n\n- Action', body)
        self.assertNotIn('Téléchargements', body)

    def test_unclassified_subject_is_not_invented_as_feature(self):
        self.assertEqual(notes.category('feat(ui): Ajout'), 'Nouveautés')
        self.assertEqual(notes.category('fix: Correction'), 'Corrections')
        self.assertEqual(notes.category('Nettoyage du code'), 'Autres changements')

    def test_repository_validation(self):
        with self.assertRaises(ValueError):
            notes.render_body('1.2.3', '', 'bad/repo/extra', None)


if __name__ == '__main__':
    unittest.main()
