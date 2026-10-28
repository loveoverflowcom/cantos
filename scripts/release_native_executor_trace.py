"""Test-only process interruption primitive; does not produce release evidence.

Call with a disposable fixture and real executor adapter. A fresh child traces one pinned
source line, stops itself before that line executes, and the parent verifies the stop and
kills that child. Host state and recovery assertions belong to the caller. Never import
this module from a production executor or interpret its success as a rehearsal PASS.
It does not terminate descendants: select only a boundary after subprocess completion and
independently verify that no owned child or writer remains before replay.
"""

import hashlib
import os
from pathlib import Path
import signal
import sys
import time


def interrupt_at_line(source, expected_sha256, function, lineno, expected_line, invoke, timeout=30, on_stopped=None):
    path = Path(source)
    if not path.is_absolute() or path != path.resolve() or not path.is_file():
        raise ValueError('canonical source file required')
    data = path.read_bytes()
    if hashlib.sha256(data).hexdigest() != expected_sha256:
        raise ValueError('source bytes do not match pin')
    lines = data.decode('utf-8').splitlines()
    if not isinstance(lineno, int) or lineno < 1 or lineno > len(lines) or lines[lineno - 1] != expected_line:
        raise ValueError('source line does not match pin')
    if not isinstance(function, str) or not function or not 0 < timeout <= 120:
        raise ValueError('bounded trace target and timeout required')
    child = os.fork()
    if child == 0:
        def trace(frame, event, arg):
            if (event == 'line' and frame.f_code.co_filename == str(path) and
                    frame.f_code.co_name == function and frame.f_lineno == lineno):
                os.kill(os.getpid(), signal.SIGSTOP)
            return trace
        try:
            sys.settrace(trace)
            invoke()
        finally:
            os._exit(70)  # Reaching completion without the intended stop is not success.
    deadline = time.monotonic() + timeout
    reaped = False
    try:
        while time.monotonic() < deadline:
            pid, status = os.waitpid(child, os.WNOHANG | os.WUNTRACED)
            if pid:
                if os.WIFSTOPPED(status) and os.WSTOPSIG(status) == signal.SIGSTOP:
                    if on_stopped is not None:
                        on_stopped()
                    os.kill(child, signal.SIGKILL)
                    killed, status = os.waitpid(child, 0)
                    reaped = True
                    if killed == child and os.WIFSIGNALED(status) and os.WTERMSIG(status) == signal.SIGKILL:
                        return {'status': 'INTERRUPTED', 'source_sha256': expected_sha256,
                                'function': function, 'line': lineno}
                    raise RuntimeError('child termination was not observed')
                reaped = not os.WIFSTOPPED(status)
                raise RuntimeError('child exited or stopped unexpectedly before target')
            time.sleep(0.01)
        raise TimeoutError('target source line was not reached')
    finally:
        if not reaped:
            try:
                os.kill(child, signal.SIGKILL)
            except ProcessLookupError:
                pass
            try:
                os.waitpid(child, 0)
            except ChildProcessError:
                pass
