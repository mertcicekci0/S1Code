#!/usr/bin/env python3
"""Offline launcher tests. All keys, subprocesses and input are fixtures."""
import unittest
from unittest.mock import patch
import types
import try_live


class LauncherTests(unittest.TestCase):
    def test_key_prefixes_belong_to_their_provider(self):
        with patch.object(try_live.getpass, 'getpass', return_value='apikey_fixture_not_real'):
            self.assertEqual(try_live.credential('fixture', 'typesafe'), 'apikey_fixture_not_real')
            with self.assertRaises(ValueError):
                try_live.credential('fixture', 'claude')
        with patch.object(try_live.getpass, 'getpass', return_value='apikey_123...abcd'):
            with self.assertRaises(ValueError):
                try_live.credential('fixture', 'typesafe')

    def run_check(self, arguments, keys, consent):
        with (patch.object(try_live.sys, 'argv', ['try_live.py', *arguments]),
              patch.object(try_live.sys.stdin, 'isatty', return_value=True),
              patch.object(try_live.sys.stdout, 'isatty', return_value=True),
              patch.object(try_live, 'prepare_checks', return_value='/fixture/live'),
              patch.object(try_live, 'base_environment', return_value={'PATH': '/fixture'}),
              patch.object(try_live.getpass, 'getpass', side_effect=keys) as prompt,
              patch('builtins.input', return_value=consent),
              patch('builtins.print'),
              patch.object(try_live.subprocess, 'run', return_value=types.SimpleNamespace(returncode=0)) as run):
            self.assertEqual(try_live.main(), 0)
            return run.call_args_list, prompt.call_count

    def test_jev_only_uses_direct_endpoint_and_no_claude_key(self):
        calls, prompts = self.run_check(['jev-check'], ['apikey_fixture_not_real'], 'RUN 1')
        self.assertEqual((len(calls), prompts), (1, 1))
        self.assertEqual(calls[0].args[0][1], 'jev_live_contract')
        self.assertEqual(calls[0].kwargs['env'], {'PATH': '/fixture', 'TYPESAFE_API_KEY': 'apikey_fixture_not_real', 'S1CODE_LIVE_BUDGET_REQUESTS': '1'})

    def test_gateway_requires_explicit_selection_and_separate_keys(self):
        calls, prompts = self.run_check(['check', '--jev-provider', 'openrouter'],
                                       ['sk-ant-api-fixture', 'sk-or-fixture'], 'RUN 2')
        self.assertEqual((len(calls), prompts), (2, 2))
        self.assertEqual(calls[1].args[0][1], 'openrouter_live_contract')
        self.assertNotIn('OPENROUTER_API_KEY', calls[0].kwargs['env'])
        self.assertNotIn('ANTHROPIC_API_KEY', calls[1].kwargs['env'])
        self.assertNotIn('TYPESAFE_API_KEY', calls[1].kwargs['env'])

    def test_claude_check_receives_selected_model_only(self):
        for arguments, model in [(['check'], 'claude-opus-5'), (['check', '--model', 'claude-sonnet-5'], 'claude-sonnet-5')]:
            calls, _ = self.run_check(arguments, ['sk-ant-api-fixture', 'apikey_fixture'], 'RUN 2')
            self.assertEqual(calls[0].kwargs['env']['S1CODE_LIVE_CLAUDE_MODEL'], model)
            self.assertNotIn('S1CODE_LIVE_CLAUDE_MODEL', calls[1].kwargs['env'])

    def test_no_consent_means_no_secrets_or_requests(self):
        calls, prompts = self.run_check(['jev-check'], [], '')
        self.assertEqual((len(calls), prompts), (0, 0))


if __name__ == '__main__':
    unittest.main()
