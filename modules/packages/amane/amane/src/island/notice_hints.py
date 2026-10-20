"""Supplement notification API with value/expire hints, correlated by reply ID.
Only notification messages are retained; no message contents are persisted.
"""
import json
import subprocess
import time


def decode(message, pending):
    payload = message.get('payload', {}).get('data', [])
    if message.get('interface') == 'org.freedesktop.Notifications' and message.get('member') == 'Notify' and len(payload) == 8:
        hint = payload[6].get('value', {})
        value = hint.get('data') if hint.get('type') in ('i', 'u', 'n', 'q', 'y') else None
        pending[(message.get('sender'), message.get('cookie'))] = max(0, min(100, value)) if isinstance(value, int) else -1
    elif message.get('type') == 'method_return':
        value = pending.pop((message.get('destination'), message.get('reply_cookie')), None)
        if value is not None and payload and isinstance(payload[0], int):
            return payload[0], value
    elif message.get('type') == 'error':
        pending.pop((message.get('destination'), message.get('reply_cookie')), None)
    if len(pending) > 256:
        pending.clear()


if __name__ == '__main__':
    while True:
        try:
            with subprocess.Popen(['busctl', '--user', '--json=short', 'monitor', 'org.freedesktop.Notifications'], stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True) as child:
                pending = {}
                for line in child.stdout:
                    try:
                        result = decode(json.loads(line), pending)
                        if result:
                            print(*result, flush=True)
                    except (ValueError, TypeError, AttributeError):
                        pass
        except OSError:
            break
        time.sleep(2)
