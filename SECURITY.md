# Security policy

CORTEX is pre-alpha and has no official stable release. Reports may concern the
current development branch; no historical version, future version, response time
or fix schedule has guaranteed support. Use the documented
[inspection trust boundary](docs/architecture/repository-inspection.md#read-only-mechanism-and-trust-boundary)
and [CLI check limits](docs/architecture/cli-check.md). The current Linux CLI is
not an execution sandbox or a general software-correctness verifier.

## Private vulnerability reporting

The approved route for CORTEX's source preview is **GitHub Private Vulnerability
Reporting** for `maikimanoel/cortex`. Before directing public reporters to it,
the maintainer must enable the feature and verify that **Security → Report a
vulnerability** is available on the repository. This policy defines the required
route; the effective GitHub setting and reporting interface establish availability.

When that private reporting route is available, use it for suspected
vulnerabilities. Include the affected commit, relevant tool/environment versions,
minimal reproduction, expected and observed behavior, and likely impact. Omit
credentials and unnecessary private data. Do not open a public issue, pull request
or discussion containing vulnerability details. If the private route is not
available, withhold public details; no alternative private contact has been
designated. This policy promises no confidentiality mechanism beyond the actual
reporting service and no response-time commitment.
