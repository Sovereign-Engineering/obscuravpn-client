# Linux CLI

Run `obscura --help` for commands. The CLI uses the service's operator-group
authorization and requires the same release as the running daemon.

## Status

`obscura status` prints the account summary and the VPN state. It first asks
the service to refresh the account information, which needs a round-trip to the
API and reports a failure when the API is unreachable.

`obscura status --local` skips that refresh and reports only what the service
already knows. The service is never asked to refresh the account, so the command
does not wait for an API round-trip. The JSON output is byte-for-byte identical
with and without `--local`; only the human-readable `Account is ...` line is
dropped.

## Exit codes

| Code | Meaning |
| --- | --- |
| 0 | Command succeeded |
| 1 | Service-reported or other command failure |
| 2 | Invalid command-line arguments |
| 3 | Service is not running |
| 4 | Operator permission denied |
| 5 | CLI and service releases differ |
| 7 | Service reported a connection error for the requested tunnel |

Service-reported failures keep code 1. IPC failures that previously returned 1
now use codes 3–5, so scripts checking specifically for 1 need updating.
Scripts checking for any nonzero result still detect failure.

`connect` waits for a matching connected tunnel. A reported connection error
ends the wait with code 7. The service keeps retrying the connection after
that; run `obscura disconnect` to stop it. If the service moves to another
target or disconnects after taking up the requested one, for example because
another client changed it, `connect` stops waiting with code 1. `disconnect`
waits for disconnection, including when the previous connection attempt
failed. Neither command cancels a daemon
request when the CLI exits; the daemon can continue acting on its target.
