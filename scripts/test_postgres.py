#!/usr/bin/env python3
"""Retired CLI: refuse every argument before probing tools, files, network or credentials.

Legacy schema/operator assets remain recovery inputs. Native business tests use run-owned
Turso files through Cargo; no disposable PostgreSQL runner is needed for source verification.
"""
import sys


def main():
    print("test_postgres.py is retired: use cargo xtask verify rust / cargo xtask verify turso. "
          "#333 native porting evidence does not authorize legacy runtime cleanup.", file=sys.stderr)
    raise SystemExit(2)


if __name__ == "__main__":
    main()
