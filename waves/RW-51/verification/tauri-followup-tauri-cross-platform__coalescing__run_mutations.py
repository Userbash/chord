from pathlib import Path
import datetime, hashlib, json, os, re, shutil, subprocess, tempfile

ROOT = Path('/root/projects/chord-modernization')
OUT = Path(__file__).parent
SOURCE = ROOT / 'rust/gui-spike/src/data.rs'
CANONICAL = SOURCE.read_text()
TEST = 'v1_voice_phases_signaling_negotiating_connected_120ms_coalescing_and_15s_phantom_timeout'
C_PROBE = OUT / 'thread_start_probe-v2.so'
ENV = dict(os.environ, CARGO_BUILD_JOBS='4', CARGO_TERM_COLOR='never',
           CARGO_TARGET_DIR=str(ROOT / 'rust/gui-spike/target'),
           PULSE_SERVER='unix:/tmp/chord-coalescing-no-audio-server')
ENV['PATH'] = '/root/.cargo/bin:' + ENV['PATH']
CASES = [
    ('control', None, None, 0),
    ('arm-inside-worker',
     '        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(120);\n        std::thread::spawn(move || {',
     '        std::thread::spawn(move || {\n            let deadline = std::time::Instant::now() + std::time::Duration::from_millis(120);', 101),
    ('fixed-relative-sleep',
     'std::thread::sleep(deadline.saturating_duration_since(std::time::Instant::now()));',
     'std::thread::sleep(std::time::Duration::from_millis(120));', 101),
    ('early-send',
     'std::thread::sleep(deadline.saturating_duration_since(std::time::Instant::now()));',
     'std::thread::sleep(std::time::Duration::ZERO);', 101),
    ('remove-sequence-guard',
     'if guard.mute_deaf_coalesce_seq == coalesce_seq\n                    && LOCAL_VOICE_ACTIVE.load(Ordering::SeqCst)',
     'if LOCAL_VOICE_ACTIVE.load(Ordering::SeqCst)', 101),
    ('wrong-final-flags',
     'let m = LOCAL_SELF_MUTE.load(Ordering::SeqCst);\n                    let d = LOCAL_SELF_DEAF.load(Ordering::SeqCst);',
     'let m = false;\n                    let d = false;', 101),
]


def log_command(phase, command, cwd, extra=None):
    stamp = datetime.datetime.now(datetime.timezone.utc).strftime('%Y%m%dT%H%M%S%fZ')
    log = OUT / f'{phase}-{stamp}.log'
    env = dict(ENV)
    if extra:
        env.update(extra)
    print(json.dumps({'phase': phase, 'command': command, 'log': str(log)}), flush=True)
    with log.open('x') as stream:
        result = subprocess.run(command, cwd=cwd, env=env, stdout=stream,
                                stderr=subprocess.STDOUT, timeout=180)
    raw = log.read_bytes()
    receipt = {'phase': phase, 'command': command, 'exit_code': result.returncode,
               'log': str(log), 'log_sha256': hashlib.sha256(raw).hexdigest()}
    log.with_suffix('.json').write_text(json.dumps(receipt, indent=2) + '\n')
    return receipt, raw.decode(errors='replace')


def resolve(binary, trace):
    text = trace.read_text()
    pcs = sorted(set(re.findall(r'0x[0-9a-f]+', text)))
    decoded = subprocess.check_output(['addr2line', '-C', '-f', '-e', str(binary), *pcs], text=True).splitlines()
    matches = [pc for i, pc in enumerate(pcs)
               if 'builder::Builder>::spawn::<chord_client::data::connect_or_update_live_voice_ext::' in decoded[i * 2]]
    assert len(matches) == 1, matches
    pc = matches[0]
    assert sum(pc in line for line in text.splitlines() if ' create ' in line) == 3
    return pc


def trace_proof(trace, delayed):
    rows = []
    selected = {}
    entered = {}
    started = {}
    returned = {}
    parent_end = {}
    for line in trace.read_text().splitlines():
        m = re.match(r'(\d+) tid=\d+ (create_delayed|create_selected|child_enter|rust_entry|rust_return|parent_pause_end) id=(\d+)', line)
        if m:
            stamp, kind, ident = int(m[1]), m[2], int(m[3])
            destination = {'create_delayed': selected, 'create_selected': selected,
                           'child_enter': entered, 'rust_entry': started,
                           'rust_return': returned, 'parent_pause_end': parent_end}[kind]
            destination[ident] = stamp
    assert len(selected) == 3, selected
    for ident in selected:
        elapsed = started[ident] - entered[ident]
        if delayed:
            assert 80 <= elapsed < 160, (ident, elapsed)
        rows.append({'id': ident, 'entry_delay_ms': elapsed,
                     'rust_return_ms': returned.get(ident), 'parent_pause_end_ms': parent_end.get(ident)})
    return {'trace': str(trace), 'sha256': hashlib.sha256(trace.read_bytes()).hexdigest(),
            'matched_children': 3, 'requested_delay_ms': 80 if delayed else 0,
            'requested_parent_pause_ms': 0 if delayed else 5, 'children': rows}


results = []
# This entire script is launched under the coordinator's single global flock.
with tempfile.TemporaryDirectory(prefix='coalescing-mutants-', dir=OUT) as tmp:
    workspace = Path(tmp) / 'rust'
    workspace.mkdir()
    shutil.copy2(ROOT / 'rust/Cargo.toml', workspace / 'Cargo.toml')
    shutil.copy2(ROOT / 'rust/Cargo.lock', workspace / 'Cargo.lock')
    ignore = shutil.ignore_patterns('target', 'node_modules', '.git')
    shutil.copytree(ROOT / 'rust/crates', workspace / 'crates', ignore=ignore)
    shutil.copytree(ROOT / 'rust/gui-spike', workspace / 'gui-spike', ignore=ignore)
    for name, before, after, expected in CASES:
        changed = CANONICAL
        if before:
            assert changed.count(before) == 1, name
            changed = changed.replace(before, after)
        (workspace / 'gui-spike/src/data.rs').write_text(changed)
        command = ['/root/.cargo/bin/cargo', 'test', '--locked', '--offline', '--manifest-path',
                   str(workspace / 'gui-spike/Cargo.toml'), '--test',
                   'server_join_and_subscriptions_contract', '--no-run', '--message-format=json-render-diagnostics']
        build, output = log_command('mutation-' + name + '-build', command, workspace)
        assert build['exit_code'] == 0, output[-3000:]
        artifacts = []
        for line in output.splitlines():
            if line.startswith('{'):
                item = json.loads(line)
                if item.get('reason') == 'compiler-artifact' and item.get('executable') and item['target']['name'] == 'server_join_and_subscriptions_contract':
                    artifacts.append(item['executable'])
        assert len(artifacts) == 1
        binary = Path(artifacts[0])
        trace = OUT / f'mutation-{name}-resolve.trace'
        control, control_text = log_command('mutation-' + name + '-resolve',
            ['unshare', '--net', 'env', 'LD_PRELOAD=' + str(C_PROBE), 'CHORD_THREAD_TRACE=' + str(trace),
             str(binary), '--exact', TEST, '--nocapture'], workspace)
        assert control['exit_code'] in (0, 101) and 'test result:' in control_text
        pc = resolve(binary, trace)
        modes = [True] if name not in ('control', 'early-send') else ([True, False] if name == 'control' else [False])
        executions = []
        for delayed in modes:
            suffix = 'delayed' if delayed else 'early-observer'
            trace = OUT / f'mutation-{name}-{suffix}.trace'
            execution, output = log_command('mutation-' + name + '-' + suffix,
                ['unshare', '--net', 'env', 'LD_PRELOAD=' + str(C_PROBE),
                 'CHORD_THREAD_TRACE=' + str(trace), 'CHORD_DELAY_CALLSITE=' + pc,
                 'CHORD_DELAY_MS=' + ('80' if delayed else '0'),
                 'CHORD_PARENT_PAUSE_MS=' + ('0' if delayed else '5'),
                 str(binary), '--exact', TEST, '--nocapture'], workspace)
            assert execution['exit_code'] == expected, output[-4000:]
            assert ('test result: ok.' if expected == 0 else 'test result: FAILED.') in output
            assert 'running 0 tests' not in output
            proof = trace_proof(trace, delayed)
            if name == 'early-send':
                assert 'Rapid mute/deaf toggles within 120ms' in output
                assert any(row['rust_return_ms'] is not None and row['parent_pause_end_ms'] is not None
                           and row['rust_return_ms'] <= row['parent_pause_end_ms'] for row in proof['children'])
            executions.append({'execution': execution, 'trace_proof': proof})
        results.append({'name': name, 'source_sha256': hashlib.sha256(changed.encode()).hexdigest(),
                        'binary_sha256': hashlib.sha256(binary.read_bytes()).hexdigest(),
                        'build': build, 'callsite_pc': pc, 'executions': executions})
        print(json.dumps({'case': name, 'killed': expected == 101}), flush=True)
assert SOURCE.read_text() == CANONICAL
report = {'control_passed': True, 'mutants_killed': 5, 'mutants_total': 5,
          'canonical_source_sha256': hashlib.sha256(CANONICAL.encode()).hexdigest(),
          'test_file_unchanged': True, 'application_sources_unchanged_by_mutations': True,
          'window_ms': 120, 'original_test_observation_ms': 160,
          'scope': 'external Linux scheduling model; exact original macOS scheduler history unobserved',
          'runs': results}
(OUT / 'mutation-summary.json').write_text(json.dumps(report, indent=2) + '\n')
print(json.dumps({'control_passed': True, 'mutants_killed': 5, 'mutants_total': 5,
                  'report': str(OUT / 'mutation-summary.json')}), flush=True)
