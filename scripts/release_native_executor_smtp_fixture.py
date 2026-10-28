"""Test-only one-message SMTP sink inside the disposable Identity network namespace.

The parent captures this process's stdout privately; message bytes are synthetic and must never
be copied to CI logs or release records. No network namespace is created or modified here.
"""
import base64
import json
import threading

from release_oci_fixture import MailSink


def main():
    sink = MailSink()
    worker = threading.Thread(target=sink.serve_forever, daemon=True)
    worker.start()
    print(json.dumps({'ready': True}), flush=True)
    try:
        message = sink.messages.get(timeout=60)
        print(json.dumps({'message': base64.b64encode(message).decode('ascii')}), flush=True)
    finally:
        sink.shutdown()
        sink.server_close()
        worker.join(timeout=10)


if __name__ == '__main__':
    main()
