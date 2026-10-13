# Service runtime work

Read [services/README.md](../../../../services/README.md) and
[ADR 0021](../../../../docs/decisions/0021-podman-service-runtime.md) before runtime edits. A service
is a process/infrastructure boundary, not necessarily a crate. Plan typed inputs and exact
ownership checks before Podman effects; use argv values, never interpolated shell commands.
Exercise foreign-resource refusal, drift, timeout, restart and retained volumes with deterministic
boundary doubles. Report live Podman evidence separately. Missing runtime/connection is
`UNSUPPORTED`; do not install it or provision a machine for a green result. Preserve native run
and disposable PostgreSQL verification; build/test must never auto-start infrastructure.


Use a unique task instance and unused port for live fixtures. Observe rootless local runtime
first; never configure a connection or machine implicitly. Keep host security labels unchanged.
Check created stopped-container ID, actual mount and volume ownership before starting it;
`nocreate` prevents accidental empty replacement. Cancellation and readiness budgets cover
the entire invocation/probe sequence. Treat timeouts as potentially ambiguous retained state.
Review recreation and upgrades separately; labels do not defend against an administrator
forging labels or force-removing resources concurrently.
