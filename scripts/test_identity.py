#!/usr/bin/env python3
"""Run the production identity tests on the host without linking OHOS/GPUI."""

import argparse
import json
from pathlib import Path
import shutil
import subprocess
import tempfile
import tomllib


def toml_value(value):
    if isinstance(value, dict):
        entries = (f"{key} = {toml_value(item)}" for key, item in value.items())
        return "{ " + ", ".join(entries) + " }"
    return json.dumps(value, ensure_ascii=False)


class IdentityTestRunner:
    def __init__(self, root):
        self.root = root
        self.dependencies = tomllib.loads((root / "Cargo.toml").read_text())["dependencies"]

    def run(self, toolchain, clippy):
        names = (
            "rcgen", "rustls", "reqwest", "serde", "serde_json", "sha2", "tempfile",
            "thiserror", "tokio", "tokio-rustls", "hyper", "hyper-util", "http-body-util", "bytes",
        )
        with tempfile.TemporaryDirectory(prefix="nearsend-identity-tests-") as directory:
            project = Path(directory)
            source = project / "src"
            source.mkdir()
            shutil.copyfile(self.root / "src/core/cert.rs", source / "cert.rs")
            shutil.copytree(self.root / "src/core/cert", source / "cert")
            (source / "lib.rs").write_text("#![cfg(test)]\nmod cert;\n")
            manifest = [
                '[package]', 'name = "nearsend-identity-tests"', 'version = "0.0.0"',
                'edition = "2021"', '', '[dependencies]',
                *(f"{name} = {toml_value(self.dependencies[name])}" for name in names),
                '', '[profile.dev]', 'debug = 0', '', '[profile.test]', 'debug = 0', '',
            ]
            (project / "Cargo.toml").write_text("\n".join(manifest))
            # Seed dependency resolution with the application's exact versions.
            shutil.copyfile(self.root / "Cargo.lock", project / "Cargo.lock")
            command = ["cargo"]
            if toolchain:
                command.append(f"+{toolchain}")
            command += [
                "clippy" if clippy else "test", "--manifest-path", str(project / "Cargo.toml"),
                "--target-dir", str(self.root / "target/identity-tests"), "-j", "4",
            ]
            if clippy:
                command += ["--tests", "--", "-D", "warnings"]
            return subprocess.run(command, cwd=self.root, check=False).returncode


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--toolchain", help="Optional rustup toolchain override")
    parser.add_argument("--clippy", action="store_true", help="Check the identity module and its tests")
    args = parser.parse_args()
    runner = IdentityTestRunner(Path(__file__).resolve().parents[1])
    return runner.run(args.toolchain, args.clippy)


if __name__ == "__main__":
    raise SystemExit(main())
