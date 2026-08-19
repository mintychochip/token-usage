# Security Policy

## Supported versions

This project is pre-1.0. Security fixes land on the default branch only.

## Reporting a vulnerability

Please **do not** open a public issue for a security problem.

Use [GitHub private vulnerability reporting](https://docs.github.com/en/code-security/security-advisories/guidance-on-reporting-and-writing/privately-reporting-a-security-vulnerability)
on this repository if it is enabled. Otherwise, open a private advisory from
the Security tab.

Include:

- A description of the issue and its impact
- Steps to reproduce, or a proof of concept
- Affected commit or version, if you know it

You should get an acknowledgement within a few days. Please give us a
reasonable window to fix and publish before any public disclosure.

## What this project stores

The local store and HTTP API persist token counts, harness names, and session
ids. Do not send prompts, transcripts, API keys, or secrets to the reporter
or API. Host wrappers should forward only usage envelopes.
