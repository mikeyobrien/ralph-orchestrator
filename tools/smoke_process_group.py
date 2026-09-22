#!/usr/bin/env python3
"""Launch or terminate the live-smoke process group on portable Unix Python."""

from __future__ import annotations

import argparse
import contextlib
import os
import signal
import time


def launch(command: list[str]) -> int:
    if not command:
        raise SystemExit("launch requires a command")
    # The caller treats this helper's PID as the process-group ID and signals it
    # to reap the whole paid-provider tree. `setsid` makes this process the
    # group leader and `execvp` replaces it in place, so the group survives
    # with the same ID. A child process would break that contract.
    os.setsid()
    # A shell here would be an injection surface and would add a process layer
    # between the group leader and the paid provider. The argv is never
    # shell-interpreted, which is the point.
    os.execvp(command[0], command)  # noqa: S606
    return 127


def group_exists(pgid: int) -> bool:
    try:
        os.killpg(pgid, 0)
    except ProcessLookupError:
        return False
    except PermissionError:
        return True
    return True


def terminate(pgid: int, grace_seconds: float) -> int:
    try:
        os.killpg(pgid, signal.SIGTERM)
    except ProcessLookupError:
        return 0

    deadline = time.monotonic() + grace_seconds
    while time.monotonic() < deadline:
        if not group_exists(pgid):
            return 0
        time.sleep(0.05)

    with contextlib.suppress(ProcessLookupError):
        os.killpg(pgid, signal.SIGKILL)
    return 0


def main() -> int:
    parser = argparse.ArgumentParser()
    subparsers = parser.add_subparsers(dest="action", required=True)
    launch_parser = subparsers.add_parser("launch")
    launch_parser.add_argument("command", nargs=argparse.REMAINDER)
    terminate_parser = subparsers.add_parser("terminate")
    terminate_parser.add_argument("pgid", type=int)
    terminate_parser.add_argument("--grace-seconds", type=float, default=10.0)
    args = parser.parse_args()

    if args.action == "launch":
        return launch(args.command)
    return terminate(args.pgid, args.grace_seconds)


if __name__ == "__main__":
    raise SystemExit(main())
