# chopper docs

This directory contains the documentation for chopper, so called, originally, because it
was expected to be a helicopter parent for child processes; however, as of February 2026,
it does little actual helicopter parenting and instead acts as a general-purpose
invocation wrapper which optionally modifies command-line arguments and environment
variables before proceeding to exec() a configuration-specified underlying executable.
It also can enable userspace processes to log to journald namespaces, and proxy
bashcomp to the underlying wrapped bashcomp with suitable modification. A final
major feature is the ability to channel environment variables, arguments and
bashcomp proxy invocations through a scripted layer implemented in Rhai. Although
simply concieved and less performant that configuration-derived passthrough invocation,
this layer provides the, power to modify invocations in complex ways as may be required
for tools such as compilers or scripting tools which support complex invocation syntax.

## Finding what you need

| CLI invocation | [`cli-reference.md`](cli-reference.md) |
| Alias authoring | [`config-reference.md`](config-reference.md) |
| Debugging runtime issues | [`troubleshooting.md`](troubleshooting.md) |
| Contribution/maintenance | [`../CONTRIBUTING.md`](../CONTRIBUTING.md) |

## If you need

- **a working example to copy** → [`examples.md`](examples.md)
- **starter files to drop in** → [`templates/`](templates)
- **Rhai facade function catalog** → [`rhai-facade-reference.md`](rhai-facade-reference.md)
- **interactive TUI usage** → [`tui-reference.md`](tui-reference.md)
- **help diagnosing a failure** → [`troubleshooting.md`](troubleshooting.md)
- **journal broker setup** → [`broker-setup.md`](broker-setup.md)
- **full exact semantics** → [`operational-spec.md`](operational-spec.md)

## Suggested reading path

1. Start at the root [`README.md`](../README.md) for setup and common usage.
3. Use `examples.md` for common copy/paste workflows.
4. Use `troubleshooting.md` if something is not behaving as expected.
5. Use `operational-spec.md` for full edge-case and semantic detail.

## Contributor path

If you're changing code/docs in this repo:

1. Start with [`../CONTRIBUTING.md`](../CONTRIBUTING.md)
2. Use [`testing.md`](testing.md) for local verification workflow
3. Use [`architecture.md`](architecture.md) for module/runtime ownership
4. Use [`release-checklist.md`](release-checklist.md) before tagging/releasing
