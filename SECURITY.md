# Security

Kestrel compiles programs and runs them. A security report is anything that
makes the compiler, the standard library or the editor do something the user did
not ask for: reading or writing the wrong file, running a command from a
program that should not, escaping a bounds or ownership check, loading a
malicious dependency, or crashing in a way that can be triggered remotely.

## Supported versions

The latest `0.1.x` release gets fixes. Older releases do not.

## Reporting

Report privately, not in a public issue:

- Open a [private security advisory](https://github.com/GeronimoCastano/kestrel/security/advisories/new)
  on GitHub, or
- email [geronimocastanorozco@hotmail.com](mailto:geronimocastanorozco@hotmail.com).

Include what you did, what happened, and the version from `kestrel --version`.
A minimal `.kes` program, a package lock or a compiler invocation is worth more
than a description. If the problem needs a specific platform, say which.

## What happens next

I read reports as they arrive and aim to reply within a week. Fixes ship in the
next patch release and the advisory names the version that fixes it. There is
no bug bounty.

## Scope

In scope:

- The compiler and its checker, proof kernel and native backend.
- The standard library, including file, process, network, Python and C interop.
- The VS Code extension and the `kestrel serve` protocol.
- The build and release pipeline.

Out of scope:

- Crashes in a C library that Kestrel links.
- Assertions that require modifying the user's own program or package.
- Denial of service through programs the user wrote and chose to run.
