#!/usr/bin/env python3
"""Retired PostgreSQL revision benchmark; direct invocation refuses before any effects.

Historical source and measurements remain attributable through Git and docs/evidence.
Native query-plan/receipt regressions live in the ordinary Server Turso tests; #384 owns
future comparable production read measurements. No old cluster or provider is required.
"""
import sys


def main():
    print("benchmark_revision_reads.py is retired; PostgreSQL benchmark setup is unavailable. "
          "Use the native Turso tests; #384 owns new read measurements.", file=sys.stderr)
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
