#!/usr/bin/env python3

"""
Run with `--help` for info.
"""

import argparse
import datetime
import json

def fmt_time(timestamp):
    return timestamp.isoformat(sep=" ", timespec="milliseconds")

def is_sleep_log(log):
    if log["eventType"] != "logEvent":
        return False

    if (
        log["subsystem"] == "com.apple.powerd"
        and log["category"] == "sleepWake"
        and "from Deep Idle" in log["eventMessage"]
    ):
        return True

    if (
        log["subsystem"] == "net.obscura.vpn-client-app.system-network-extension"
        and (
            "wake entry" in log["eventMessage"]
            or "sleep exit" in log["eventMessage"]
        )
    ):
        return True

    return False

if __name__ == "__main__":
    parser = argparse.ArgumentParser(
        description="Extracts messages related to sleeping as well as printing periods of no logs (when the machine is presumably asleep.",
    )
    parser.add_argument("path")
    parser.add_argument(
        "-s",
        "--min-seconds",
        default=60,
        help="Minimum idle duration to log.",
    )
    args = parser.parse_args()

    max_sleep = datetime.timedelta()
    max_sleep_time = None
    last_entry = None

    noteworthy = datetime.timedelta(seconds=args.min_seconds)

    with open(args.path) as f:
        for line in f:
            log = json.loads(line)

            strtimestamp = log.get("timestamp")
            if not strtimestamp:
                continue

            timestamp = datetime.datetime.fromisoformat(strtimestamp)

            if is_sleep_log(log):
                print(f"{fmt_time(timestamp)} {log.get("eventMessage")}")

            if last_entry is not None:
                delta = timestamp - last_entry

                if delta > max_sleep:
                    max_sleep = delta
                    max_sleep_time = timestamp

                if delta >= noteworthy:
                    print(f"{fmt_time(timestamp)} sleep for {delta}")

            last_entry = timestamp

    print(f"max sleep {max_sleep} at {fmt_time(max_sleep_time)}")
