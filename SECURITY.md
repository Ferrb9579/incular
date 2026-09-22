# Security policy

Please do not publish an unpatched security vulnerability in a public issue.

Use [GitHub's private report form](https://github.com/Ferrb9579/incular/security/advisories/new).
Include the affected version/commit, platform, enabled features, reproduction,
impact and proposed mitigation. Minimize data and review attachments first.

During release preparation this repository's private reporting setting was
disabled. The maintainer must enable it in repository security settings and
verify the form before publication. If the form is unavailable, open an issue
only requesting that a private reporting channel be enabled; **do not include
vulnerability details or attachments in that public request**. No unverified
private email address is advertised.

## Supported versions and response

Before the first release, fixes target the current development branch. After
publication, the latest patch of the current experimental 0.x minor line receives
security fixes. Older lines have no backport promise; any exceptions will be
stated in an advisory. The maintainer aims to acknowledge a private report within
seven days; this is a target, not a support SLA. Coordinate disclosure after a
fix or mitigation is available, and publish affected/fixed versions clearly.

Security fixes should include a regression test where practical. Do not put
credentials, private keys, access tokens, or sensitive user data in reports.

DevTools is an authenticated local diagnostic interface, not a network service.
See [diagnostics privacy](docs/devtools.md) for discovery credentials, text,
screenshots and crash dumps, and [dependency risks](docs/dependency-risks.md)
for tracked maintenance advisories. Secrets accidentally published must be
revoked; yanking a crate does not remove its contents.
