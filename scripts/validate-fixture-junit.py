#!/usr/bin/env python3
"""Reject empty, skipped, errored, or failing CI fixture JUnit reports."""
from pathlib import Path
import sys
import xml.etree.ElementTree as ET


def element_name(element):
    return element.tag.rsplit('}', 1)[-1]


def validate_report(path):
    try:
        root = ET.parse(path).getroot()
    except ET.ParseError as error:
        raise ValueError(f'malformed JUnit XML: {error}') from error
    if element_name(root) not in {'testsuite', 'testsuites'}:
        raise ValueError('JUnit root must be testsuite or testsuites')
    cases = 0
    for element in root.iter():
        name = element_name(element)
        if name in {'skipped', 'error', 'failure'}:
            raise ValueError(f'fixture report contains {name}')
        for key in ('tests', 'failures', 'errors', 'skipped', 'disabled'):
            if key not in element.attrib:
                continue
            raw = element.attrib[key]
            if not raw or not raw.isascii() or not raw.isdigit():
                raise ValueError(f'invalid JUnit {key} count: {raw!r}')
            if key != 'tests' and int(raw) != 0:
                raise ValueError(f'fixture report has {key}={raw}')
        if name == 'testcase':
            if not element.attrib.get('name', '').strip():
                raise ValueError('JUnit testcase must identify an actual test')
            if any(element_name(child) in {'testcase', 'testsuite', 'testsuites'} for child in element):
                raise ValueError('JUnit testcase cannot contain test containers')
            cases += 1
    if cases == 0:
        raise ValueError('fixture report contains no actual test cases')
    return cases


def main(argv=None):
    args = sys.argv[1:] if argv is None else argv
    if len(args) != 1:
        print('usage: validate-fixture-junit.py <report.xml>', file=sys.stderr)
        return 2
    try:
        cases = validate_report(Path(args[0]))
    except (OSError, ValueError) as error:
        print(f'fixture-junit: {error}', file=sys.stderr)
        return 1
    print(f'fixture-junit: accepted {cases} test cases; no skips, failures, or errors')
    return 0


if __name__ == '__main__':
    sys.exit(main())
