#!/usr/bin/env python3
"""Measure an OHOS app's CPU time without top's startup sampling bias."""

import argparse
import json
import subprocess
import time
from pathlib import Path


class CpuMeasurement:
    def __init__(self, target, bundle):
        self.command = ["hdc", "-t", target, "shell"]
        self.pid = self.read("pidof", bundle).strip()
        if not self.pid.isdigit():
            raise RuntimeError(f"No single running process for {bundle}: {self.pid}")

    def read(self, *args):
        return subprocess.check_output(self.command + list(args), text=True).strip()

    def sample(self):
        stat = self.read("cat", f"/proc/{self.pid}/stat")
        # Fields after comm start at field 3 (state); utime/stime are 14/15.
        fields = stat[stat.rfind(")") + 2 :].split()
        return time.monotonic(), int(fields[11]) + int(fields[12])

    def run(self, count, interval):
        samples = []
        start_time, start_ticks = previous = self.sample()
        for _ in range(count):
            time.sleep(interval)
            current = self.sample()
            elapsed = current[0] - previous[0]
            # OHOS/Linux procfs exposes CPU time in USER_HZ (100 ticks/s).
            samples.append(round((current[1] - previous[1]) / elapsed, 3))
            previous = current
        elapsed = previous[0] - start_time
        return {
            "pid": int(self.pid),
            "duration_seconds": round(elapsed, 3),
            "cpu_seconds": (previous[1] - start_ticks) / 100,
            "cpu_percent_one_core": round((previous[1] - start_ticks) / elapsed, 3),
            "samples_cpu_percent_one_core": samples,
        }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", required=True)
    parser.add_argument("--bundle", default="com.richerfu.nearsend")
    parser.add_argument("--label", required=True)
    parser.add_argument("--count", type=int, default=3)
    parser.add_argument("--interval", type=float, default=10)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    result = CpuMeasurement(args.target, args.bundle).run(args.count, args.interval)
    result.update(target=args.target, bundle=args.bundle, label=args.label)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result))


if __name__ == "__main__":
    main()
