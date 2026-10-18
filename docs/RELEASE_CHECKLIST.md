# Preview release checklist

Release target: **0.1.0 preview**, source available under PolyForm Noncommercial 1.0.0. This is a preview, not a claim of production readiness or an independent security audit.

## Completed locally

- [x] License, readable project overview, service setup, contribution and security guidance.
- [x] Linux Secret Service support and OpenGL/EGL fallback alongside Vulkan.
- [x] Windows executable resources and portable ZIP packaging; Linux tarball/desktop entry; macOS app packaging.
- [x] Native build/test/package CI configured for macOS, Windows, and Ubuntu.
- [x] Matching version tags automate publication after all checks/builds pass, with eight packages and a checksum manifest. Apple Silicon ZIP and DMG built locally; DMG integrity and generated bundle versions verified. Other native packaging jobs still need their first GitHub run.
- [x] Dependency license report generated for the supported desktop targets, included in packages.
- [x] Rust unit/regression suite: 31 passed, one optional sample-writing test ignored. Covers graph validity, group duplication, import safety, credential encryption/integrity/legacy compatibility, Office formats, attachments, MIME, routing response validation, and clean-profile UI smoke checks.
- [x] Code formatting and strict Clippy checks on macOS.
- [x] macOS first-use UI: empty home, project creation, blank canvas, category expansion, adding blocks, click-to-connect, Play → End completion without unlocking credentials.
- [x] Native macOS import and export of the bundled example, preserving both canvases and blocks; starter workflow creation and property editing.
- [x] Common API-token/private-key signature scan of the working tree and all three existing commits: no matches. This is a targeted check, not proof that all personal data or secrets are absent.
- [x] RustSec audit: ZIP upgraded from 8.6.0 to 9.0.5 to resolve RUSTSEC-2026-0367, -0368, and -0369. No known vulnerability findings remain in the lockfile audit. Five transitive maintenance warnings remain (`paste`, `ttf-parser`, `zbus_names`, `zvariant`, `zvariant_derive`); track their upstream replacements. The old `block` dependency also emits a future Rust compatibility warning.

The optional ignored Office sample-writing test creates inspection files; it is not part of the normal unit suite. Provider behavior and email delivery were not exercised by these offline tests.

## Before publishing downloadable builds

- [ ] Push and inspect the three native CI jobs; download and test their artifacts. No Windows or Linux native build has been run on the development Mac.
- [ ] Windows desktop: first launch, Credential Manager save/reload, file dialogs, pan/zoom, Gmail browser callback, generated Office files.
- [ ] Linux desktop: X11 and Wayland, Secret Service lock/unlock/save/reload, portal file dialogs, Vulkan and OpenGL fallback, Gmail callback.
- [ ] Test a disposable account end to end: Jev routes, uncertain decision approval, Tavily research, a generated document, editable Gmail reply, explicit send approval. Confirm no duplicate send on Pause/Reset.
- [ ] Repeat trackpad pinch/scroll, node dragging, connection drag/drop menus, and large-graph responsiveness with real input hardware. The UI automation pass could verify click-to-connect but did not establish native drag/trackpad behavior.
- [ ] Decide whether to ship unsigned/ad-hoc signed previews with clear install instructions, or add Windows signing and macOS Developer ID/notarization.
- [ ] Enable GitHub private vulnerability reporting and review repository visibility, author metadata, issues, and release notes.
- [ ] Review exported workflow prompts and paths before sharing examples. Repeat the repository/history secret scan after preparing the release commit.
- [ ] Build a release from the exact reviewed commit, attach checksums and platform/architecture labels, and create a draft release before publishing.
