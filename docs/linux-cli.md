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

## Output formats

`obscura status` prints a human-readable summary. `--json` prints the status
payload the service reports, as one pretty-printed JSON object. `--json-lines`
prints the same payload as one compact JSON object per line, one physical line
per record. `--json` and `--json-lines` cannot be used together.

`--json-lines` is the machine-readable stream format. With `--follow` it prints
one record per status change the service publishes. `--json --follow` prints
repeated pretty-printed objects back to back, which is not a parseable stream.

The payload keeps the service's own shape: the outer fields are camelCase while
the exit and relay fields inside them are snake_case. This schema is not
frozen; these commands preserve today's bytes so that scripts written against
them keep working.

The status payload is sensitive. It carries the account identifier, the
account's referral code and funding and subscription details, the configured
API URL, the enabled feature flags and their keys, the pinned and last-chosen
locations, and the WireGuard public keys of the connected exit and entry relay.
Select the fields you need before writing status output to a log, a file, or a
ticket.

## Waiting for a connection

`connect` and `disconnect` wait until the service reaches the requested state.
`--no-wait` returns as soon as the service accepts the request, so a script can
observe the change with `status` instead of blocking. `--timeout SECONDS`
bounds the whole operation, including sending the request and waiting for the
state. `--no-wait` and `--timeout` cannot be used together.

With `--timeout`, a connection error does not end the wait. The service keeps
retrying, so the CLI waits until the tunnel connects or the deadline expires.
Without `--timeout`, a connection error ends the wait with code 7.

A timeout is reported with exit code 6. The outcome is unknown at that point,
because the service may still apply the requested state. Read the status
instead of repeating the command: the CLI never sends a compensating
disconnect and never retries the request.

## The exit catalogue

`obscura locations list` prints the exits the service has cached. The list is
sorted by country, then city, then exit ID, and it labels the exit ID and the
provider ID separately:

```
US  nyc  exit NYC-001  New York  provider us-nyc-wg-301
```

`obscura locations list --json` prints the service's cache wrapper, not a
flattened list: `version` is a base64 string, `last_updated` is the unix time
in seconds, and the exits are under `value.exits`. The wrapper is preserved so
that the CLI output matches the payload an API consumer sees. This schema is
not frozen; the command preserves today's bytes.

The service waits for a catalogue to exist, so the read is bounded: it waits at
most 5 seconds, and `--timeout SECONDS` sets another limit between 1 and 3600.
When the limit expires, the command reports exit code 6 and names
`obscura locations refresh` as the next step. A read never refreshes the
catalogue.

`obscura locations refresh` is the explicit network operation: it asks the
service to fetch the catalogue from the API and update its cache. It waits at
most 30 seconds, and `--timeout SECONDS` sets another limit between 1 and 3600.
A refresh that times out reports exit code 6; the catalogue may or may not have
been updated, so read it with `locations list` instead of refreshing again.

## Choosing a location

`obscura connect` connects to any exit. The selection flags narrow that down:

```
obscura connect --country SE
obscura connect --country se --city sto
obscura connect --exit NYC-001
```

`--city` needs `--country`. `--exit` cannot be combined with `--country` or
`--city`. A country code is two ASCII letters. A blank value, or a value with
control characters, is rejected before the command runs.

The selection is resolved against the catalogue the service has cached, so run
`obscura locations refresh` first when it is empty. `--country` and `--city`
are matched ignoring case, and the command sends the canonical values from the
catalogue. `--exit` is matched exactly, against the exit ID, not the provider
ID: `us-nyc-wg-301` in the example above is a provider ID and is not an exit.

A selection that no exit matches fails with exit code 1, before the tunnel is
changed: the command sends no request to change the tunnel. The message names
`obscura locations list` as the next step.

The catalogue lookup shares the deadline of the operation. With `--timeout
SECONDS` the lookup is bounded by that deadline, and a lookup that expires
reports exit code 6. Without it, the lookup waits at most 5 seconds, like
`obscura locations list` does. A connection without a selection never reads the
catalogue.

## Settings

`obscura auto-connect` and `obscura lan` read and change the settings the
service already has:

```
obscura auto-connect get
obscura auto-connect set on
obscura auto-connect set off
obscura lan get
obscura lan set allow
obscura lan set block
```

A read reports the value the service has. `obscura auto-connect get` prints
`Auto-connect is on.` and `obscura lan get` prints `Local network access is
allowed.` With `--json` a read prints a named projection with only that
setting in it: `{"autoConnect":true}` or `{"localNetworkAccess":true}`.

A read asks the service for the same status that `obscura status --local`
reports. It never asks the service to refresh the account information, so it
does not wait for an API round-trip, and it never changes a setting.

A change sends the setting to the service, which stores it the way the
desktop app does. The command changes that setting and nothing else: it does
not connect, disconnect, restart the service or write the configuration file,
and it never asks for elevated rights. A failure that the service reports
exits with code 1, and a denied permission exits with code 4, like the other
commands.

Local network access is a routing decision. Allowing it changes the routes and
the firewall rules of the tunnel, so a change can alter the network policy of
a tunnel that is already connected. The command reports the state it set; it
does not decide whether that is acceptable for the network it runs on.

## Exit codes

| Code | Meaning |
| --- | --- |
| 0 | Command succeeded |
| 1 | Service-reported or other command failure |
| 2 | Invalid command-line arguments |
| 3 | Service is not running |
| 4 | Operator permission denied |
| 5 | CLI and service releases differ |
| 6 | Operation timed out; the outcome is unknown |
| 7 | Service reported a connection error for the requested tunnel |

Service-reported failures keep code 1. IPC failures that previously returned 1
now use codes 3–5, so scripts checking specifically for 1 need updating.
Scripts checking for any nonzero result still detect failure.

Code 6 covers the connection timeouts, the catalogue timeouts and the
catalogue lookup of a connect request. A timed-out catalogue read leaves the
cache unchanged; a timed-out catalogue refresh may have updated it.

`connect` waits for a matching connected tunnel. A reported connection error
ends the wait with code 7. The service keeps retrying the connection after
that; run `obscura disconnect` to stop it. If the service moves to another
target or disconnects after taking up the requested one, for example because
another client changed it, `connect` stops waiting with code 1. `disconnect`
waits for disconnection, including when the previous connection attempt
failed. Neither command cancels a daemon
request when the CLI exits; the daemon can continue acting on its target.
