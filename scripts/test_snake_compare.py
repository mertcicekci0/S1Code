"""Offline comparison invariants. Never starts a provider or reads credentials."""
import hashlib
import functools
import http.server
import json
import pathlib
import tempfile
import threading
import types
import urllib.request
import urllib.error
import unittest
from unittest.mock import patch
import snake_compare as compare


class ComparisonTests(unittest.TestCase):
    def test_preview_refuses_metadata_and_symlinks(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            (root/'index.html').write_text('game fixture')
            (root/'.git').mkdir()
            (root/'.git/config').write_text('private fixture metadata')
            (root/'outside.txt').write_text('not a game asset')
            (root/'styles.css').symlink_to(root/'outside.txt')
            class Quiet(compare.GameHandler):
                def log_message(self, *_args):
                    pass
            server = http.server.ThreadingHTTPServer(('127.0.0.1',0), functools.partial(Quiet,directory=str(root)))
            thread = threading.Thread(target=server.serve_forever,daemon=True)
            thread.start()
            url = 'http://127.0.0.1:' + str(server.server_port)
            try:
                with urllib.request.urlopen(url+'/index.html',timeout=2) as response:
                    self.assertEqual(response.read(),b'game fixture')
                for path in ['/.git/config','/%2egit/config','/outside.txt','/styles.css']:
                    with self.assertRaises(urllib.error.HTTPError) as caught:
                        urllib.request.urlopen(url+path,timeout=2)
                    self.assertEqual(caught.exception.code,404)
                    caught.exception.close()
            finally:
                server.shutdown()
                server.server_close()
                thread.join(timeout=2)

    def test_same_prompt_and_model_without_blanket_permission_bypass(self):
        prompt = 'Build Snake.\nRun the tests.'
        for arm in ['claude', 's1code']:
            argv = compare.command(arm, pathlib.Path('/fixture/repo'), pathlib.Path('/fixture'),
                                   'claude-fixture', prompt, 24)
            self.assertEqual(argv.count(prompt), 1)
            self.assertEqual(argv[argv.index('--model') + 1], 'claude-fixture')
            self.assertFalse(any('skip-permissions' in a for a in argv))
            self.assertNotIn('--approve', argv)
            self.assertNotIn('--auto-approve', argv)
        argv = compare.command('s1code', pathlib.Path('/fixture/repo'), pathlib.Path('/fixture'),
                               'claude-fixture', prompt, 24, auto_approve=True)
        self.assertIn('--auto-approve', argv)
        for budget in [None, 0, -1, 201]:
            with self.assertRaises(ValueError):
                compare.command('s1code', pathlib.Path('/fixture'), pathlib.Path('/fixture'),
                                'claude-fixture', prompt, budget)

    def test_changed_prompt_stops_before_key_access_or_process_launch(self):
        with tempfile.TemporaryDirectory() as directory:
            base = pathlib.Path(directory)
            (base/'prompt.txt').write_text('changed')
            manifest = {'directory':directory, 'prompt_sha256':hashlib.sha256(b'original').hexdigest()}
            with (patch.object(compare.sys.stdin, 'isatty', return_value=True),
                  patch.object(compare.sys.stdout, 'isatty', return_value=True),
                  patch.object(compare, 'Keychain') as keys,
                  patch.object(compare.subprocess, 'run') as run):
                with self.assertRaisesRegex(RuntimeError, 'prompt changed'):
                    compare.run('s1code', manifest, 24)
                keys.assert_not_called()
                run.assert_not_called()

    def test_claude_uses_managed_auth_without_loading_native_keys(self):
        with tempfile.TemporaryDirectory() as directory:
            base = pathlib.Path(directory)
            (base/'prompt.txt').write_text('Build Snake')
            manifest = {'directory':directory, 'prompt_sha256':hashlib.sha256(b'Build Snake').hexdigest(),
                        'starting_commit':'fixture-commit', 'model':'claude-fixture'}
            with (patch.object(compare.sys.stdin, 'isatty', return_value=True),
                  patch.object(compare.sys.stdout, 'isatty', return_value=True),
                  patch.object(compare, 'Keychain') as keys,
                  patch.object(compare.shutil, 'which', return_value='/fixture/claude'),
                  patch.object(compare.subprocess, 'check_output', side_effect=[b'',b'fixture-commit']),
                  patch.object(compare.subprocess, 'run', return_value=types.SimpleNamespace(returncode=0)),
                  patch.object(compare.subprocess, 'call', return_value=0) as call,
                  patch.dict(compare.os.environ, {'ANTHROPIC_API_KEY':'fixture-secret','TYPESAFE_API_KEY':'fixture-secret'}),
                  patch('builtins.print')):
                compare.run('claude', manifest, None)
                keys.assert_not_called()
                self.assertNotIn('ANTHROPIC_API_KEY', call.call_args.kwargs['env'])
                self.assertNotIn('TYPESAFE_API_KEY', call.call_args.kwargs['env'])
            report = json.loads((base/'records/claude.json').read_text())
            self.assertIsNone(report['cost_usd'])
            self.assertFalse(report['gameplay_verified'])


if __name__ == '__main__':
    unittest.main()
