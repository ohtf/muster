# Security policy

Panic Button is a staff panic alarm. A vulnerability that stops alarms being delivered, forges or suppresses alarms, or misleads a user about whether an alarm was delivered is a safety issue as well as a security issue.

## Reporting a vulnerability

**Do not report suspected vulnerabilities in public issues, discussions or pull requests.**

Report them privately, either:

- through GitHub's private vulnerability reporting on this repository (the *Security* tab, then *Report a vulnerability*); or
- by email to [security@openhealthtechnology.org](mailto:security@openhealthtechnology.org).

Include a description and likely impact, affected versions or commits, safe reproduction steps and any mitigation you can suggest. If the issue could affect whether an alarm is raised, delivered or shown honestly, say so clearly: it is triaged as both a security finding and a clinical safety hazard (see [SAFETY.md](SAFETY.md)).

We aim to acknowledge reports within 5 working days and to give a substantive response within 15 working days.

## Disclosure

We use coordinated disclosure with a 90-day default window from the initial report. Once a fix is available we publish an advisory and release note and, if you agree, credit you. The full OHTF policy is at <https://openhealthtechnology.org/community/security/>.

## Scope

All code in this repository, including the desktop agent, headless agent, CLI and relay. Relay operators other than OHTF are responsible for their own deployments, but please tell us about anything in the relay software itself.
