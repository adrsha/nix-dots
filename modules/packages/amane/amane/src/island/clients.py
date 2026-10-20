"""Read-only Mango snapshot decoder; the Rust UI owns all actions.

Use the standard JSON decoder (including escaped Unicode/window titles) rather
than growing a partial JSON parser in the config's dependency-free crate.
"""
import json
import os
import socket
import time


def records(snapshot):
    clients = snapshot.get("clients")
    if not isinstance(clients, list):
        return None
    result = []
    for c in clients:
        if not isinstance(c, dict) or c.get("is_swallowedby"):
            continue
        ident, tags = c.get("id"), c.get("tags", [])
        if not isinstance(ident, int) or not isinstance(tags, list):
            continue
        tags = [str(t) for t in tags if isinstance(t, int) and 1 <= t <= 31]
        fields = [c.get("monitor", ""), c.get("appid", ""), c.get("title", "")]
        result.append("\t".join([str(ident), ",".join(tags)] +
                                [str(f).encode().hex() for f in fields]))
    return result


if __name__ == "__main__":
    path = os.environ.get("MANGO_INSTANCE_SIGNATURE")
    if path:
        while True:
            try:
                with socket.socket(socket.AF_UNIX) as connection:
                    connection.connect(path)
                    connection.sendall(b"watch all-clients\n")
                    for line in connection.makefile():
                        try:
                            rows = records(json.loads(line))
                            if rows is not None:
                                print("SNAP", *rows, "END", sep="\n", flush=True)
                        except (ValueError, TypeError):
                            pass
            except OSError:
                pass
            print("SNAP\nEND", flush=True)
            time.sleep(1)
