# Security Policy

## Supported versions

| Version | Supported |
|---------|-----------|
| 0.1.x   | Yes       |
| < 0.1   | No        |

Hyper is an early public release (`0.1.x`). Security fixes target the latest `0.1.x` line on `main`.

## Reporting a vulnerability

**Do not** open a public GitHub issue for security vulnerabilities.

Please report privately using one of:

1. **[GitHub Security Advisories](https://github.com/muhammadyusufpov/hyper/security/advisories/new)** (preferred)
2. Email: [muhammadyusuf.yusupov201@gmail.com](mailto:muhammadyusuf.yusupov201@gmail.com)

Include:

- Hyper version or commit (`hyper` build / `git rev-parse HEAD`)
- OS and how you run Hyper (`run`, `compile`, `--emit-exe`)
- Steps to reproduce
- Impact (crash, unexpected code execution, data exposure, etc.)

You should receive an acknowledgment within a few days. We will work with you on a fix and coordinated disclosure when appropriate.

## What is not a security vulnerability

Please use [regular issues](https://github.com/muhammadyusufpov/hyper/issues) for:

- Language design questions and feature requests
- Compiler / interpreter bugs that do not have a security impact
- Documented gaps in [known limitations](doc/compiler/known-limitations.md)
- Documentation typos and CI failures

When in doubt, report privately — we can reclassify as a normal issue if needed.
