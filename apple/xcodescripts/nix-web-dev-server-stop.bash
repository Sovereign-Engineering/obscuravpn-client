#!/usr/bin/env bash

# This just kills whatever is using the port. It is ugly but the best option for a few reasons.
# 1. Xcode ignores the result of pre-actions. This means that we have no way to signal a failure.
# 2. The clean action will destroy our PID file.
#
# So we want the best chance of succeeding or the user will unknowingly be using a stale web server. The only way to reliably free up the port is to kill what is listening on it.

# TODO: remove magic 1420 port
kill "$(lsof -ti 'tcp:1420')"
