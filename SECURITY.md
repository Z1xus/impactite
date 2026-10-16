# Security policy

## Supported versions

Security fixes are released for the latest version only.

## Reporting a vulnerability

Do not report vulnerabilities in public issues. Submit a [private security advisory](https://github.com/Z1xus/impactite/security/advisories/new) or email [hi@z1x.us](mailto:hi@z1x.us).

Include:

- The affected version (`impactite --version`)
- The deployment method (binary, Docker or behind a reverse proxy)
- Steps to reproduce and the impact

Reports are acknowledged within 72 hours. Reporters are credited in the release notes unless they ask otherwise.

## Scope

In scope:

- Authentication or authorization bypass, including access to another account, team or site
- Access to a password-protected share without the password
- Remote code execution, file access or denial of service from a request, an import file or a tracker event
- Cross-site scripting in the dashboard or on share pages

Out of scope:

- Fake pageviews or events. The tracker endpoints accept anonymous events by design.
- Missing security headers without a demonstrated attack
