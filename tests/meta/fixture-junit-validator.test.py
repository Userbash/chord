"""CI-only stdlib regressions for the fixture JUnit acceptance contract."""
import importlib.util
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / 'scripts/validate-fixture-junit.py'
SPEC = importlib.util.spec_from_file_location('fixture_junit_validator', SCRIPT)
VALIDATOR = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(VALIDATOR)


class FixtureJunitContract(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory(prefix='fixture-junit-')
        self.addCleanup(self.directory.cleanup)
        self.report = Path(self.directory.name) / 'report.xml'

    def validate(self, xml):
        self.report.write_text(xml, encoding='utf8')
        return VALIDATOR.validate_report(self.report)

    def test_real_leaf_cases_are_required_even_when_suite_counts_claim_success(self):
        for xml in ['<testsuites/>', '<testsuite tests="7" failures="0" errors="0" skipped="0"/>']:
            with self.subTest(xml=xml), self.assertRaises(ValueError):
                self.validate(xml)

    def test_passed_plain_and_nested_cases_are_accepted(self):
        xml = '<testsuites><testcase name="plain"/><testsuite tests="1" failures="0" errors="0" skipped="0"><testcase name="nested"/></testsuite></testsuites>'
        self.assertEqual(self.validate(xml), 2)

    def test_skipped_case_is_rejected_despite_zero_summary_counts(self):
        with self.assertRaises(ValueError):
            self.validate('<testsuite tests="1" skipped="0"><testcase name="case"><skipped/></testcase></testsuite>')

    def test_error_case_is_rejected_despite_zero_summary_counts(self):
        with self.assertRaises(ValueError):
            self.validate('<testsuite tests="1" errors="0"><testcase name="case"><error>crash</error></testcase></testsuite>')

    def test_failure_case_is_rejected_despite_zero_summary_counts(self):
        with self.assertRaises(ValueError):
            self.validate('<testsuite tests="1" failures="0"><testcase name="case"><failure>assertion</failure></testcase></testsuite>')

    def test_nonzero_suite_or_root_outcome_counts_are_rejected(self):
        for tag in ['testsuites', 'testsuite']:
            for count in ['skipped', 'failures', 'errors', 'disabled']:
                with self.subTest(tag=tag, count=count), self.assertRaises(ValueError):
                    self.validate(f'<{tag} {count}="1"><testcase name="case"/></{tag}>')

    def test_malformed_or_negative_counts_are_not_success(self):
        for count in ['tests', 'skipped', 'failures', 'errors', 'disabled']:
            for value in ['-1', 'NaN', '0.5', '']:
                with self.subTest(count=count, value=value), self.assertRaises(ValueError):
                    self.validate(f'<testsuite {count}="{value}"><testcase name="case"/></testsuite>')

    def test_malformed_xml_and_unrelated_roots_fail_closed(self):
        for xml in ['not XML', '<testsuites>', '<report><testcase name="case"/></report>']:
            with self.subTest(xml=xml), self.assertRaises(ValueError):
                self.validate(xml)

    def test_cli_missing_report_returns_nonzero_without_success_output(self):
        result = subprocess.run([sys.executable, str(SCRIPT), str(self.report)], capture_output=True, text=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(result.stdout, '')
        self.assertIn('fixture-junit:', result.stderr)

    def test_cli_rejects_empty_error_failure_and_malformed_reports(self):
        reports = ['<testsuites/>', '<testsuites>',
                   '<testsuites><testcase name="case"><error/></testcase></testsuites>',
                   '<testsuites><testcase name="case"><failure/></testcase></testsuites>']
        for xml in reports:
            with self.subTest(xml=xml):
                self.report.write_text(xml, encoding='utf8')
                result = subprocess.run([sys.executable, str(SCRIPT), str(self.report)], capture_output=True, text=True)
                self.assertNotEqual(result.returncode, 0)
                self.assertEqual(result.stdout, '')
                self.assertIn('fixture-junit:', result.stderr)

    def node_report(self, source):
        test_file = Path(self.directory.name) / 'sample.test.mjs'
        test_file.write_text(source, encoding='utf8')
        node = os.environ.get('NODE_BINARY', 'node')
        return subprocess.run([node, '--test', '--test-reporter=junit', str(test_file)], capture_output=True, text=True)

    def test_actual_node_junit_report_is_accepted_with_real_cases(self):
        source = 'import {test,describe} from "node:test"; import assert from "node:assert/strict"; test("plain",()=>assert.equal(2+3,5)); describe("nested",()=>test("child",()=>assert.deepEqual([1,2].length,2)));'
        result = self.node_report(source)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.validate(result.stdout), 2)
        cli = subprocess.run([sys.executable, str(SCRIPT), str(self.report)], capture_output=True, text=True)
        self.assertEqual(cli.returncode, 0, cli.stderr)
        self.assertIn('2 test cases', cli.stdout)

    def test_node_can_exit_zero_for_skips_but_validator_rejects_the_report(self):
        source = 'import {test} from "node:test"; test("intentional validator negative fixture",{skip:"report check"},()=>{throw new Error("must not execute");});'
        result = self.node_report(source)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.report.write_text(result.stdout, encoding='utf8')
        cli = subprocess.run([sys.executable, str(SCRIPT), str(self.report)], capture_output=True, text=True)
        self.assertNotEqual(cli.returncode, 0)
        self.assertEqual(cli.stdout, '')


if __name__ == '__main__':
    unittest.main(verbosity=2)
