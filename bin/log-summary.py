#!/usr/bin/env python3

import argparse
import datetime
import json
import re
import sys
import zoneinfo

LEVELS = {
    "Debug": 0,
    "Info": 1,
    "Default": 2,
    "Error": 3,
    "Fault": 4,
}

LEVEL_MAX = 5 # Higher than all levels.

RUST_PRINT_RE = re.compile("|".join([
    "creating tunnel  .*",
    "deriving connect error code for tunnel (creation|connect): .*",
    "finishing tunnel connection  .*",
    "Ignoring failure to update exit list: .*",
    "Selected exit  .*",
    "selected relay  .*",
    "tunnel connected",
    '"preferred network path interface name:.*',
    '"sleep entry .*',
    '"startTunnel entry .*',
    '"stopTunnel entry .*',
    '"wake entry .*',
    '.* message_id="(3rOUXFti|Azzlo6j2|KT91bgvI|OfLfwKhf|TJ4nH30h|uQ0xQcPP|UROUZerU)".*',
]), re.DOTALL)

SWIFT_PRINT_RE = re.compile("|".join([
    "NWPathMonitor event: .*",
]))

def format_log_time(log):
    date = datetime.datetime.fromisoformat(log["timestamp"])
    return format_time(date)

def format_time(date):
    if args.zone == "":
        return ""

    r = ""

    for zone in args.zone.split(","):
        if zone == "local":
            converted = date.astimezone(None)
        elif zone == "source":
            converted = date
        elif zone == "utc":
            converted = date.astimezone(datetime.timezone.utc)
        else:
            converted = date.astimezone(zoneinfo.ZoneInfo(zone))

        if args.date:
            r += converted.strftime("%Y-%m-%d %H:%M:%S.%f")[:-3] + " "
        else:
            r += converted.strftime("%H:%M:%S.%f")[:-3] + " "

    return r

if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("path")
    parser.add_argument(
        "-d",
        "--date",
        action="store_true",
        help="Show the date along with the time."
    )
    parser.add_argument(
        "-l",
        "--level",
        choices=list(LEVELS),
        default="Fault",
        help="Print all logs at or above this level",
    )
    parser.add_argument(
        "-z",
        "--zone",
        default="source",
        help="A comma separated list of timezones in which to display times. Each item is either `source` (for the users timezone), `local` for your timezone or an IANNA timezone name (like `America/Toronto`)."
    )
    args = parser.parse_args()

    min_level = LEVELS[args.level]

    with open(args.path) as f:
        for line in f:
            entry = json.loads(line)

            if entry["eventType"] != "logEvent":
                continue

            subsystem = entry["subsystem"]

            if subsystem == "net.obscura.rust-apple":
                msg = entry["eventMessage"]
                if RUST_PRINT_RE.match(msg):
                    print(format_log_time(entry), msg)
                elif ' message_id="eech6Ier"' in msg:
                    print(format_log_time(entry), "Racing relays... CONNECTION ATTEMPT START")
                elif 'Ignoring failure to update exit list' in msg:
                    print("WTF", msg)
                    print("MATCH", RUST_PRINT_RE.match(msg))
                elif LEVELS.get(entry["messageType"], LEVEL_MAX) >= min_level:
                    print(format_log_time(entry), msg)
            elif subsystem == "net.obscura.vpn-client-app":
                msg = entry["eventMessage"]
                if SWIFT_PRINT_RE.match(msg):
                    print(format_log_time(entry), msg)
            elif subsystem == "" and entry["processID"] == 0:
                msg = entry["eventMessage"]
                if msg == "PMRD: trace point 0x18":
                    print(format_log_time(entry), "########## KERNEL SLEEP ##########")
