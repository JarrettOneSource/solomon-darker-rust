"""Bounded diagnostic controls over the retained test executable, never a package."""
import json
import os
import pathlib
import subprocess
import time

reports = pathlib.Path('reports/windows-transport-unit')
binary = (reports / 'test-binary.exe').resolve()
slow = 'transport::tests::a_slow_reader_keeps_ordered_messages_and_receives_the_current_snapshot_after_resuming'
shutdown = 'transport::tests::critical_events_survive_pressure_and_shutdown_interrupts_incomplete_requests'
controls = [('slow-default', slow, {}, 6),
            ('slow-constrained-buffers', slow, {'SOLOMON_TRANSPORT_CONSTRAIN_BUFFERS': '1'}, 2),
            ('shutdown-default', shutdown, {}, 6),
            ('shutdown-forced-early-accept', shutdown, {'SOLOMON_TRANSPORT_FORCE_EARLY_ACCEPT': '1'}, 1)]
rows = []
for mode, test, changes, repeats in controls:
    for iteration in range(repeats):
        env = {**os.environ, 'SOLOMON_TRANSPORT_DIAGNOSTIC': '1', **changes}
        for key in ['SOLOMON_TRANSPORT_CONSTRAIN_BUFFERS', 'SOLOMON_TRANSPORT_FORCE_EARLY_ACCEPT']:
            if key not in changes:
                env.pop(key, None)
        started = time.monotonic()
        try:
            result = subprocess.run([str(binary), '--exact', test, '--nocapture', '--test-threads=1'],
                env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=90)
            code, output, timed_out = result.returncode, result.stdout, False
        except subprocess.TimeoutExpired as error:
            code, output, timed_out = None, error.output or b'', True
        filename = f'{mode}-{iteration + 1}.log'
        (reports / filename).write_bytes(output[:262144])
        row = {'mode': mode, 'iteration': iteration + 1, 'test': test, 'exitCode': code,
               'timedOut': timed_out, 'elapsedMs': round((time.monotonic() - started) * 1000),
               'retainedLogBytes': min(len(output), 262144), 'totalOutputBytes': len(output), 'log': filename}
        rows.append(row)
        (reports / 'results.json').write_text(json.dumps({'status': 'diagnostic-in-progress', 'results': rows}, indent=2) + '\n')
        print(json.dumps(row), flush=True)
failed = sum(row['exitCode'] != 0 for row in rows)
(reports / 'results.json').write_text(json.dumps({'status': 'diagnostic-complete', 'failedControls': failed,
    'scope': 'Original assertions unchanged; constrained sockets and forced pending accept are named diagnostic controls, not qualification.',
    'results': rows}, indent=2) + '\n')
raise SystemExit(1 if failed else 0)
