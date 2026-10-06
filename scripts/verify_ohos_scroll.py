#!/usr/bin/env python3
"""Capture OHOS scroll continuity and hold/release checks (uv --with pillow)."""

import argparse
import json
import subprocess
import time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

from PIL import Image, ImageChops, ImageStat


class ScrollVerification:
    def __init__(self, target, kind, output):
        self.command = ["hdc", "-t", target]
        self.kind = kind
        self.output = output
        output.mkdir(parents=True, exist_ok=True)
        self.x, self.top, self.bottom = (
            (660, 700, 2200) if kind == "phone" else (2200, 650, 1450)
        )
        self.crop = (80, 180, 1220, 2450) if kind == "phone" else (1000, 370, 2530, 1640)
        self.initial_pid = self.shell("pidof", "com.richerfu.nearsend")
        self.initial_faults = set(self.shell("ls", "-1", "/data/log/faultlog/faultlogger").splitlines())
        self.rows = []

    def run_command(self, *args):
        result = subprocess.run(self.command + list(map(str, args)), capture_output=True, text=True, check=True)
        if "[Fail]" in result.stdout or "[Fail]" in result.stderr:
            raise RuntimeError(result.stdout + result.stderr)
        return result.stdout.strip()

    def shell(self, *args):
        return self.run_command("shell", *args)

    def input(self, *args):
        result = self.shell("uitest", "uiInput", *args)
        if "No Error" not in result:
            raise RuntimeError(result)

    def screenshot(self, label):
        remote = f"/data/local/tmp/nearsend-scroll-{self.kind}.png"
        result = self.shell("uitest", "screenCap", "-p", remote)
        if "saved" not in result:
            raise RuntimeError(result)
        local = self.output / f"{self.kind}-{label}.png"
        self.run_command("file", "recv", remote, local)
        return local

    def difference(self, a, b):
        with Image.open(a) as first, Image.open(b) as second:
            diff = ImageChops.difference(first.convert("RGB").crop(self.crop), second.convert("RGB").crop(self.crop))
            return round(sum(ImageStat.Stat(diff).mean) / 3, 6)

    def hold_release(self, label, direction):
        y = (self.top + self.bottom) // 2
        self.shell("uinput", "-T", "-m", self.x, y, self.x, y + direction * 180, "-k", 350, 600)
        first = self.screenshot(label + "-released")
        time.sleep(.7)
        second = self.screenshot(label + "-later")
        mean = self.difference(first, second)
        self.rows.append({"scenario": label, "post_release_mean_pixel_difference": mean, "held_position": mean < .05})
        print(self.kind, label, mean, flush=True)

    def continuous_swipe(self, label, direction, velocity):
        y1, y2 = (self.bottom, self.top) if direction < 0 else (self.top, self.bottom)
        with ThreadPoolExecutor(max_workers=1) as executor:
            motion = executor.submit(self.input, "swipe", self.x, y1, self.x, y2, velocity)
            captures = []
            deadline = time.monotonic() + 3.5
            while time.monotonic() < deadline:
                captures.append(self.screenshot(f"{label}-frame-{len(captures):02}"))
                if motion.done() and len(captures) >= 8:
                    break
                time.sleep(.05)
            motion.result()
        self.rows.append({"scenario": label, "velocity": velocity, "direction": direction, "frames": len(captures)})
        print(self.kind, label, len(captures), flush=True)

    def run(self):
        self.screenshot("settings-start")
        self.hold_release("hold-up", -1)
        self.hold_release("hold-down", 1)
        self.continuous_swipe("slow-up", -1, 500)
        self.continuous_swipe("slow-down", 1, 500)
        self.continuous_swipe("fast-up", -1, 3500)
        self.continuous_swipe("fast-down", 1, 3500)
        self.input("fling", self.x, self.bottom, self.x, self.top, 3500, 100)
        y = (self.top + self.bottom) // 2
        self.shell("uinput", "-T", "-d", self.x, y, "-i", 200, "-u", self.x, y)
        caught = self.screenshot("caught-fling")
        time.sleep(.7)
        caught_later = self.screenshot("caught-fling-later")
        mean = self.difference(caught, caught_later)
        self.rows.append({"scenario": "catch-fling", "post_release_mean_pixel_difference": mean, "held_position": mean < .05})
        for i in range(24 if self.kind == "phone" else 16):
            direction = -1 if i % 2 == 0 else 1
            y = (self.top + self.bottom) // 2
            self.shell("uinput", "-T", "-m", self.x, y, self.x, y + direction * 180, 250)
            time.sleep(.2)
            self.screenshot(f"repeat-{i:02}")
            self.rows.append({"scenario": f"repeat-{i}", "same_pid": self.shell("pidof", "com.richerfu.nearsend") == self.initial_pid})
            print(self.kind, "repeat", i, flush=True)
        faults = sorted(set(self.shell("ls", "-1", "/data/log/faultlog/faultlogger").splitlines()) - self.initial_faults)
        result = {"kind": self.kind, "target": self.command[2], "rows": self.rows, "new_faults": faults,
                  "same_pid": self.shell("pidof", "com.richerfu.nearsend") == self.initial_pid}
        result["automated_checks_passed"] = not faults and result["same_pid"] and all(row.get("held_position", True) and row.get("same_pid", True) for row in self.rows)
        (self.output / f"{self.kind}-scroll-checks.json").write_text(json.dumps(result, indent=2) + "\n")
        print(json.dumps({key: value for key, value in result.items() if key != "rows"}), flush=True)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", required=True)
    parser.add_argument("--kind", choices=["phone", "2in1"], required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    ScrollVerification(args.target, args.kind, args.output).run()
