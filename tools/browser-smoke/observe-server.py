"""Private timing observations around the unchanged canonical stdlib server."""
import collections
import faulthandler
import json
import os
from pathlib import Path
import runpy
import sys
import threading
import time

output = Path(os.environ['SERVER_OBSERVATION'])
output.mkdir(parents=True, exist_ok=True)
events = collections.deque()
functions = {
    'get_request', 'process_request', 'process_request_thread', 'finish_request',
    'handle_one_request', 'parse_request', 'do_GET', 'do_HEAD', 'send_head',
    'translate_path', 'guess_type', 'send_response', 'log_request', 'log_message',
    'end_headers', 'flush_headers', 'copyfile', 'finish',
}

def observe(frame, event, argument):
    filename = frame.f_code.co_filename.replace('\\', '/')
    if not (filename.endswith('/http/server.py') or filename.endswith('/socketserver.py')):
        return
    name = frame.f_code.co_name
    if event in ('c_call', 'c_return', 'c_exception'):
        called = getattr(argument, '__name__', '')
        if called not in ('open', 'stat', 'fstat', 'sendall', 'write', 'flush', 'accept'):
            return
        name += ':' + called
    elif event not in ('call', 'return') or name not in functions:
        return
    handler = frame.f_locals.get('self')
    events.append({'time': time.time_ns() / 1e6, 'monotonic': time.monotonic_ns() / 1e6,
                   'thread': threading.get_native_id(), 'event': event, 'function': name,
                   'path': getattr(handler, 'path', None),
                   'client': getattr(handler, 'client_address', None)})

def write_events():
    with (output / 'server-phases.jsonl').open('w') as log:
        while True:
            while events:
                log.write(json.dumps(events.popleft(), separators=(',', ':')) + '\n')
            log.flush()
            time.sleep(.5)

threading.Thread(target=write_events, daemon=True).start()
stacks = (output / 'server-stacks.log').open('w')
faulthandler.dump_traceback_later(15, repeat=True, file=stacks)
threading.setprofile(observe)
sys.setprofile(observe)
runpy.run_path('scripts/serve-client.py', run_name='__main__')
