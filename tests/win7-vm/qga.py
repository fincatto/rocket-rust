#!/usr/bin/env python3
"""Cliente mínimo do QEMU Guest Agent (socket unix) para o E2E Win7.

uso:
  qga.py <socket> ping
  qga.py <socket> push <arquivo-local> <caminho-no-guest>
  qga.py <socket> pull <caminho-no-guest> <arquivo-local>
  qga.py <socket> exec <timeout-s> <programa> [args...]   # imprime stdout/stderr, sai com o exit code
"""

import base64
import json
import socket
import sys
import time

CHUNK = 48 * 1024


class Agent:
    def __init__(self, path: str, timeout: float = 30.0):
        self.sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        self.sock.settimeout(timeout)
        self.sock.connect(path)
        self.buf = b""
        self._sync()

    def _sync(self) -> None:
        # Descarta respostas antigas pendentes no canal.
        token = int(time.time()) & 0x7FFFFFFF
        self.sock.sendall(b"\xff" + json.dumps({"execute": "guest-sync-delimited", "arguments": {"id": token}}).encode())
        while True:
            resp = self._read()
            if resp.get("return") == token:
                return

    def _read(self) -> dict:
        while b"\n" not in self.buf:
            data = self.sock.recv(65536)
            if not data:
                raise ConnectionError("guest agent fechou a conexão")
            self.buf += data
        line, self.buf = self.buf.split(b"\n", 1)
        line = line.lstrip(b"\xff").strip()
        return json.loads(line) if line else self._read()

    def call(self, cmd: str, **args):
        msg = {"execute": cmd}
        if args:
            msg["arguments"] = args
        self.sock.sendall(json.dumps(msg).encode())
        resp = self._read()
        if "error" in resp:
            raise RuntimeError(f"{cmd}: {resp['error'].get('desc', resp['error'])}")
        return resp.get("return")

    def push(self, local: str, remote: str) -> None:
        h = self.call("guest-file-open", path=remote, mode="wb")
        try:
            with open(local, "rb") as f:
                while chunk := f.read(CHUNK):
                    self.call("guest-file-write", handle=h, **{"buf-b64": base64.b64encode(chunk).decode()})
        finally:
            self.call("guest-file-close", handle=h)

    def pull(self, remote: str, local: str) -> None:
        h = self.call("guest-file-open", path=remote, mode="rb")
        try:
            with open(local, "wb") as f:
                while True:
                    r = self.call("guest-file-read", handle=h, count=CHUNK)
                    f.write(base64.b64decode(r["buf-b64"]))
                    if r.get("eof") or r["count"] == 0:
                        break
        finally:
            self.call("guest-file-close", handle=h)

    def exec(self, timeout: float, path: str, args: list[str]) -> int:
        pid = self.call("guest-exec", path=path, arg=args, **{"capture-output": True})["pid"]
        deadline = time.time() + timeout
        while time.time() < deadline:
            st = self.call("guest-exec-status", pid=pid)
            if st["exited"]:
                for key, stream in (("out-data", sys.stdout), ("err-data", sys.stderr)):
                    if key in st:
                        stream.write(base64.b64decode(st[key]).decode(errors="replace"))
                return st.get("exitcode", 1)
            time.sleep(1)
        print(f"timeout após {timeout}s: {path} {args}", file=sys.stderr)
        return 124


def main() -> int:
    if len(sys.argv) < 3:
        print(__doc__, file=sys.stderr)
        return 2
    sock, cmd, rest = sys.argv[1], sys.argv[2], sys.argv[3:]
    agent = Agent(sock)
    if cmd == "ping":
        agent.call("guest-ping")
        return 0
    if cmd == "push" and len(rest) == 2:
        agent.push(*rest)
        return 0
    if cmd == "pull" and len(rest) == 2:
        agent.pull(*rest)
        return 0
    if cmd == "exec" and len(rest) >= 2:
        return agent.exec(float(rest[0]), rest[1], rest[2:])
    print(__doc__, file=sys.stderr)
    return 2


if __name__ == "__main__":
    sys.exit(main())
