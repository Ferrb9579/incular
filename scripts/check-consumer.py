"""Build a fresh consumer from .crate archives through a loopback staging registry.

No workspace paths or patches are used. External index entries and downloads
come from crates.io. Run scripts/release.py package first. Never publishes.
"""
import functools
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile
import threading
import urllib.error
import urllib.request

import release

ROOT = release.ROOT


class Registry(BaseHTTPRequestHandler):
    archives = {}
    indexes = {}

    def log_message(self, *_args):
        pass

    def respond(self, data, content_type):
        self.send_response(200)
        self.send_header("Content-Type", content_type)
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def do_GET(self):
        path = self.path.split("?", 1)[0]
        if path == "/index/config.json":
            base = f"http://127.0.0.1:{self.server.server_port}"
            self.respond(json.dumps({"dl": base + "/crates/{crate}/{version}/download"}).encode(), "application/json")
            return
        if path.startswith("/index/"):
            key = path[len("/index/"):]
            if not re.fullmatch(r"[a-zA-Z0-9_/-]+", key):
                self.send_error(400)
                return
            if key in self.indexes:
                self.respond(self.indexes[key], "text/plain")
                return
            try:
                with urllib.request.urlopen("https://index.crates.io/" + key, timeout=60) as response:
                    self.respond(response.read(), "text/plain")
            except urllib.error.HTTPError as error:
                self.send_error(error.code)
            except (urllib.error.URLError, TimeoutError):
                self.send_error(502)
            return
        match = re.fullmatch(r"/crates/([a-zA-Z0-9_-]+)/([a-zA-Z0-9_.+-]+)/download", path)
        if match:
            name, version = match.groups()
            filename = f"{name}-{version}.crate"
            if filename in self.archives:
                self.respond(self.archives[filename].read_bytes(), "application/octet-stream")
            else:
                self.send_response(302)
                self.send_header("Location", f"https://static.crates.io/crates/{name}/{filename}")
                self.end_headers()
            return
        self.send_error(404)


def main():
    release.archives()
    packages = release.publishable()
    facade = next(package for package in packages if package["name"] == "incular")
    for package in packages:
        name = package["name"]
        key = f"{name[:2]}/{name[2:4]}/{name}"
        Registry.indexes[key] = (ROOT / "target/package/tmp-registry/index" / key).read_bytes()
        filename = f"{name}-{package['version']}.crate"
        Registry.archives[filename] = ROOT / "target/package" / filename
    server = ThreadingHTTPServer(("127.0.0.1", 0), Registry)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        with tempfile.TemporaryDirectory(prefix="consumer-", dir=ROOT / "target") as directory:
            consumer = Path(directory)
            (consumer / "src").mkdir()
            (consumer / ".cargo").mkdir()
            (consumer / ".cargo/config.toml").write_text(
                '[source.crates-io]\nreplace-with = "release-staging"\n'
                f'[source.release-staging]\nregistry = "sparse+http://127.0.0.1:{server.server_port}/index/"\n', encoding="utf-8")
            (consumer / "Cargo.toml").write_text(
                '[package]\nname = "incular-release-consumer"\nversion = "0.0.0"\nedition = "2024"\n'
                f'rust-version = "{facade["rust_version"]}"\npublish = false\n[workspace]\n'
                f'[dependencies]\nincular = {{ version = "={facade["version"]}", default-features = false }}\n'
                '[features]\ndefault = ["desktop"]\ndesktop = ["incular/desktop", "incular/controls", "incular/material"]\n'
                'devtools = ["incular/devtools"]\n[[bin]]\nname = "hello"\npath = "src/main.rs"\nrequired-features = ["desktop"]\n', encoding="utf-8")
            (consumer / "src/lib.rs").write_text(
                'use incular::prelude::*;\npub fn label() -> Widget { Text::new("Registry consumer").into() }\n', encoding="utf-8")
            (consumer / "src/main.rs").write_bytes((ROOT / "crates/incular/examples/hello.rs").read_bytes())
            env = dict(os.environ, CARGO_TARGET_DIR=str(ROOT / "target/release-consumer"))
            run = functools.partial(subprocess.run, cwd=consumer, env=env, check=True)
            run(["cargo", "check", "--lib", "--no-default-features"])
            run(["cargo", "build", "--bin", "hello", "--locked"])
            run(["cargo", "check", "--all-targets", "--all-features", "--locked"])
            print("Fresh registry consumer passed: minimal library, desktop hello, all features.")
    finally:
        server.shutdown()
        server.server_close()
        thread.join()


if __name__ == "__main__":
    main()
