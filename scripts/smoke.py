#!/usr/bin/env python3
"""Keyboard/PTY acceptance test against the disposable Compose fixture.

Run cargo build, docker compose up -d --wait, and install dev/requirements-test.txt.
Creates and deletes one uniquely named customer through the application's edit UI.
"""
import codecs
import fcntl
import json
import os
from pathlib import Path
import pty
import select
import struct
import subprocess
import tempfile
import termios
import time
import uuid

import pyte

os.chdir(Path(__file__).resolve().parent.parent)


class Tui:
    def __init__(self, *args, database_args=None):
        self.master, slave = pty.openpty()
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 36, 160, 0, 0))
        # Deliberately targets the local Compose fixture, ignoring DATABASE_URL.
        url = f"postgresql://ultorg:ultorg@localhost:{os.environ.get('POSTGRES_PORT', '55432')}/ultorg"
        self.process = subprocess.Popen(
            ["target/debug/ultorglike", *(database_args or ["--database-url", url]), *args],
            stdin=slave, stdout=slave, stderr=slave,
            env={**os.environ, "TERM": "xterm-256color"},
        )
        os.close(slave)
        self.screen = pyte.Screen(160, 36)
        self.stream = pyte.Stream(self.screen)
        self.decoder = codecs.getincrementaldecoder("utf8")()

    def read(self, seconds=0.15):
        end = time.monotonic() + seconds
        while time.monotonic() < end:
            if select.select([self.master], [], [], 0.02)[0]:
                try:
                    self.stream.feed(self.decoder.decode(os.read(self.master, 65536)))
                except OSError:
                    break
        return "\n".join(self.screen.display)

    def send(self, text):
        os.write(self.master, text.encode())
        return self.read()

    def wait(self, text):
        deadline = time.monotonic() + 15
        while time.monotonic() < deadline:
            output = self.read()
            if text in output and "Loading…" not in output and "Applying transaction…" not in output:
                return output
            if self.process.poll() is not None:
                break
        raise AssertionError(f"Expected {text!r}:\n{self.read()}")

    def command(self, text):
        return self.send(":" + text + "\r")

    def close(self):
        if self.process.poll() is None:
            self.process.terminate()
            self.process.wait(timeout=5)
        os.close(self.master)


def postgres_workflow():
    with tempfile.TemporaryDirectory(prefix="ultorg-smoke-") as directory:
        directory = Path(directory)
        session = directory / "workspace.json"
        app = Tui("--view", "examples/customer-totals.json", "--editable")
        fixture_id = 1_000_000 + uuid.uuid4().int % 1_000_000_000
        fixture_name = f"PTY fixture {fixture_id}"
        created = False
        try:
            app.wait("1109.325")
            app.command("limit 1")
            app.wait("497.685")
            app.send("7l")
            app.wait("line_items → quantity")
            app.send("]p")
            app.wait("rows 2–2")
            app.send("[p")
            app.wait("rows 1–1 +")
            app.send("+")
            app.wait("rows 1–2")
            for command, expected in [("query", "Page size:"), ("sql", "Generated SQL"), ("schema", "Primary key:")]:
                app.command(command)
                app.wait(expected)
                if command == "sql":
                    app.send("G")
                    app.wait("$2")
                    app.send("g")
                app.send("q")
            app.send("K")
            app.wait("Type:")
            app.send("q")
            app.command(f"session_save {session}")
            app.wait("Saved workspace")
            app.command("duplicate")
            app.wait("view 2/2")
            app.command("close")
            app.wait("view 1/1")
            app.command(f"export json {directory / 'rows.json'}")
            app.wait("Exported loaded results")
            exported = json.loads((directory / "rows.json").read_text())
            assert exported["partial"] and exported["scope"] == "loaded"
            assert exported["rows"][0]["fields"]["total_spend"].startswith("1109.325")
            app.command(f"export csv {directory / 'rows.csv'}")
            app.wait(str(directory / "rows.csv"))
            assert "customers.orders.total" in (directory / "rows.csv").read_text()
            app.command("open customers")
            app.wait("view 2/2")
            app.command(f"filter customers.id = {fixture_id}")
            app.wait("No matching rows")
            app.command("insert " + json.dumps({"id": str(fixture_id), "company_name": fixture_name, "country": "Norway"}))
            app.wait("1 staged")
            app.command("apply")
            app.wait("Staged database changes")
            app.send("q")
            created = True
            app.command("apply yes")
            app.wait(fixture_name)
            app.command('edit "PTY changed 東京"')
            app.wait("* PTY changed 東京")
            app.command("discard")
            app.wait(fixture_name)
            app.command('edit "PTY changed 東京"')
            app.command("apply yes")
            app.wait("0 staged")
            app.wait("PTY changed 東京")
            app.send("dd")
            app.wait("1 staged")
            app.command("apply yes")
            app.wait("No matching rows")
            created = False
            app.send("q")
            app.process.wait(timeout=5)
            assert app.process.returncode == 0
            print("PASS: nested paging, inspection, export, tabs, insert/update/delete, discard, quit")
        finally:
            app.close()
            if created:
                # Cleanup is restricted to this run's uniquely named fixture row.
                subprocess.run(["docker", "compose", "exec", "-T", "postgres", "psql", "-U", "ultorg", "-d", "ultorg", "-c", f"DELETE FROM customers WHERE id={fixture_id}"], check=False, capture_output=True)
        restored = Tui("--session", str(session))
        try:
            restored.wait("rows 1–2")
            restored.wait("line_items → quantity")
            restored.send("q")
            restored.process.wait(timeout=5)
            assert restored.process.returncode == 0
            print("PASS: workspace startup restores selected nested collection and page window")
        finally:
            restored.close()


if __name__ == "__main__":
    postgres_workflow()
