#!/usr/bin/env python3
"""Real terminal workflow for both embedded backends; only temporary files used."""
import json
from pathlib import Path
import subprocess
import tempfile

from smoke import Tui

for backend in ["sqlite", "duckdb"]:
    with tempfile.TemporaryDirectory(prefix=f"ultorg-{backend}-") as directory:
        directory = Path(directory)
        database = directory / f"fixture with spaces.{backend}"
        session = directory / "workspace.json"
        subprocess.run(["target/debug/examples/seed", backend, str(database)], check=True)
        args = [f"--{backend}", str(database)]
        app = Tui("--editable", database_args=args)
        try:
            app.wait("東京商店")
            app.command("open customers")
            app.wait("東京商店")
            app.command("join orders customers")
            app.wait("Shipped")
            app.command("join items orders")
            app.wait("items")
            app.command("formula orders.total = sum([items.price])")
            app.wait("total")
            app.command("limit 1")
            app.wait("rows 1–1 +")
            app.command("sql")
            app.wait("Root collection")
            app.send("G")
            app.send("q")
            app.command(f"export json {directory / 'data.json'}")
            app.wait("Exported loaded results")
            data = json.loads((directory / "data.json").read_text())
            assert data["partial"]
            assert data["rows"][0]["fields"]["name"] == "東京商店"
            app.command(f"session_save {session}")
            app.wait("Saved workspace")
            app.command("open customers")
            app.wait("東京商店")
            app.command("filter customers.id = 99")
            app.wait("No matching rows")
            app.command('insert {"id":"99","name":"terminal fixture"}')
            app.wait("1 staged")
            app.command("apply yes")
            app.wait("terminal fixture")
            app.command('edit "terminal changed"')
            app.wait("1 staged")
            app.command("apply yes")
            app.wait("0 staged")
            app.wait("terminal changed")
            app.send("dd")
            app.wait("1 staged")
            app.command("apply yes")
            app.wait("No matching rows")
            app.send("q")
            app.process.wait(timeout=5)
            assert app.process.returncode == 0
        finally:
            app.close()
        restored = Tui("--session", str(session), database_args=args)
        try:
            restored.wait("東京商店")
            restored.wait("rows 1–1 +")
            restored.send("q")
            restored.process.wait(timeout=5)
            assert restored.process.returncode == 0
        finally:
            restored.close()
        print(f"PASS: {backend} CLI, nested views, formulas, paging, SQL, export, workspace, insert/update/delete")
