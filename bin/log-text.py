#!/usr/bin/env python3

import argparse
import datetime
import json
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

LEVEL_FMT = {
    "Debug": "D",
    "Info": "I",
    "Default": "L",
    "Error": "E",
    "Fault": "F",

    None: "N",
    "unknown": "U",
}

IGNORED_TYPES = {
    "activityCreateEvent",
    "signpostEvent",
    "stateEvent",
    "unknown",
    "userActionEvent",
}

OUR_PROCESSES = {
    "Obscura VPN",
    "net.obscura.vpn-client-app.system-network-extension",
}

UI_SUBSYSTEMS = {
    "com.apple.AppKit",
    "com.apple.CFBundle",
    "com.apple.defaults",
}

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

def format_log(log):
    if log.get("finished") == 1:
        return "Finished"

    if log["eventType"] in IGNORED_TYPES:
        return None

    if log["eventType"] == "timesyncEvent":
        date = datetime.datetime.fromisoformat(log["timestamp"])
        datestr = format_time(date)
        return f"{datestr}timesyncEvent"

    if args.obscura and log["processImagePath"] not in OUR_PROCESSES:
        return None

    level_int = LEVELS.get(log["messageType"], LEVEL_MAX)
    if level_int < min_level:
        return None

    if not args.ui and log["subsystem"] in UI_SUBSYSTEMS:
        return None

    date = datetime.datetime.fromisoformat(log["timestamp"])
    datestr = format_time(date)

    level_s = LEVEL_FMT.get(log["messageType"], "?")

    return f"{datestr}{level_s} {log["processImagePath"]}:{log["subsystem"]}:{log["category"]} | {log["eventMessage"]}"

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
        default="Debug",
        help="Minimum log level to print.",
    )
    parser.add_argument(
        "--obscura",
        action="store_true",
        help="Show only logs from our processes."
    )
    parser.add_argument(
        "--ui",
        action="store_true",
        help="Show UI-related logs."
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

            formatted = format_log(entry)
            if formatted == None:
                continue

            print(formatted)
