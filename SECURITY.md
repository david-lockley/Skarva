# Security

Skarva is an early desktop preview. Only the latest repository version receives fixes; it has not had an independent security audit.

Report vulnerabilities privately through [GitHub's security advisory form](https://github.com/david-lockley/skarva/security/advisories/new). If private reporting is unavailable, open a public issue asking for a private contact without including exploit details, credentials, or personal data.

Include the affected version, platform, reproduction steps using invented data, and the likely impact. Never include real tokens, the vault master key, private email, or an application-data archive.

The application sends workflow context to configured providers and can send Gmail replies after explicit approval. Treat imported workflows, retrieved sources, and email as untrusted. OS key storage and encrypted vault files do not protect credentials from malware that can inspect the running process.
