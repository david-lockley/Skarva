# Skarva

An agentic whiteboard. Build connected workflows on a native Rust canvas using email, AI, research, and document blocks; Jev chooses between connected paths.

## Run the app

Install Rust and run `cargo run --release`. On macOS, `sh scripts/package-macos.sh release` creates `target/release/Skarva.app`. On Windows, `cargo build --release` creates `target/release/skarva.exe` with the app icon and product information embedded. The Windows build requires the Windows SDK resource compiler (MSVC) or MinGW windres (GNU).

The app uses native GPU rendering: Metal on macOS, DirectX 12 or Vulkan on Windows.

## Brand assets

The original Figma exports are `assets/app-icon.png` (gradient background), `assets/logo-mark.png` (transparent), and `assets/logo-mark.svg` (vector). The gradient is reserved for the OS icon; the app uses the transparent mark on its charcoal interface.

Generated assets include macOS `packaging/macos/AppIcon.icns`, Windows `packaging/windows/app-icon.ico`, a small window icon, and a cropped transparent sidebar mark. To regenerate them after replacing the source images, install Pillow and run `python3 scripts/generate-icons.py`. Normal Rust builds use the included assets and do not require Python.

## Build a workflow

Create a project from the home screen. Project cards show a canvas preview; use the card's **…** menu to rename or delete a project. You can also click the project title in the editor to rename it.

Start with **Email received** to watch Gmail, or **Play** to run manually. Enter the manual starting message in the Play block's properties. Add blocks from the sidebar or drag a connection into empty space to create the next step.

Press **Play** in the toolbar. The app infers how to run from the starting block and opens the relevant integration settings if a connection is missing. If a canvas has several starting blocks, select the one to run. The same button pauses or resumes a run; Reset clears its activity and highlighted path.

A simple email workflow is **Email received → AI model → Reply to email → End**. A document workflow can be **Email received → Web research → Word → AI model → Reply to email → End**. The reply introduces the generated document and attaches it automatically. Every email waits for your review and **Approve & send**.

**Word**, **Excel**, and **PowerPoint** create editable DOCX, XLSX, and PPTX files. Their properties describe what to create. **AI model** writes or transforms text using the configured provider. **Local file** reads a selected reference file each time the workflow runs.

When a block has several outgoing connections, Jev chooses one path. Use **Choice** for named paths or **Yes / No** for a binary question. Enter the decision question in the block properties. Each answer can connect to multiple destinations, from which Jev chooses one. Routing settings let you adjust when the app should pause and ask you to choose.

## Connect services

Open **Integrations** from the home screen or editor. All service settings are in this panel:

- **AI model:** choose OpenRouter, OpenAI, or Anthropic, enter a model ID and the provider's API key, and save. OpenRouter's `openrouter/free` is an option for initial experiments.
- **Jev decisions:** enter the TypeSafe API key from [the TypeSafe console](https://console.typesafe.ai/). It is needed when a workflow branches.
- **Web research:** enter a [Tavily API key](https://app.tavily.com/). Research blocks use a basic search and pass source excerpts and URLs to later blocks.
- **Gmail:** save a Google Cloud Desktop OAuth client under Google OAuth setup, then click **Connect Gmail**.

For Gmail, enable the Gmail API in Google Cloud, configure an External OAuth consent screen in Testing mode, and add your Gmail address as a test user. Add the `gmail.readonly` and `gmail.send` scopes, then create a Desktop app OAuth client. See [Google's desktop OAuth guide](https://developers.google.com/identity/protocols/oauth2/native-app). Google OAuth testing tokens may expire after seven days; reconnect when needed. Public distribution with Gmail read access requires Google's verification.

Connections are stored in an AES-256-GCM encrypted vault in the app's local data folder. A single random master key stays in macOS Keychain or Windows Credential Manager. The app reads that key once per launch and keeps the unlocked vault in memory; saving credentials and refreshing Gmail tokens do not access Keychain again. Failed unlocks are cached for the launch so canceling a prompt cannot trigger a cascade of prompts. Reopen the app to retry an unlock.

On upgrading from the separate-key storage, paste API keys and Google OAuth client settings into Integrations again and reconnect Gmail once. Old protected entries are left untouched and never read automatically, since importing them would request access to each item again. Project files never contain credentials.

The vault remains encrypted on disk, with owner-only file permissions on macOS. The security tradeoff is keeping all connections unlocked in the app's memory for the session. Malware that can inspect or inject into the running app can steal those credentials; an encrypted vault does not protect a compromised running process.

The example project in [examples/streamlined-assistant.json](examples/streamlined-assistant.json) demonstrates both entry points: Quick document runs Play → Word, while Email assistant chooses research, an Office file, or a direct reply from the email itself. Files and conversation context flow automatically, with no explicit memory or variable blocks.

## Automatic context

Outputs flow through the connected path without variable wiring. Later steps receive the incoming message, readable attachments, local reference files, research results, previous AI output, and generated document content.

Context is remembered automatically. Email workflows load the previous saved result for the same conversation in the same canvas, and update it after an approved reply is sent. Manual workflows remember their last completed run in that canvas. Each stored result is bounded; this is recent context rather than a complete conversation archive.

Existing Recall and Remember blocks are removed automatically when projects load, and their incoming and outgoing connections are joined. The original project JSON is retained beside the saved project with the suffix `.json.before-automatic-memory`.

Incoming attachments and Local file support TXT, Markdown, CSV, TSV, JSON, PDF, DOCX, XLSX, and PPTX. Up to five email attachments of 5 MB each are considered. Scanned PDFs need OCR, which is not included. Local file blocks save a path; reselect the file if moving the project to another computer.

Email, file contents, and remembered context go to the configured AI provider and to Jev when deciding a path. Research sends an AI-generated search query to Tavily. Source excerpts and incoming content are untrusted input.

## Canvas controls

- Drag blocks to move them. Drag empty space to box-select; Ctrl/Command-click adds or removes blocks from the selection.
- Drag an output port to another block to connect it. Release over empty space to create and connect a new block.
- Click a connection and press Delete/Backspace to remove it.
- Ctrl/Command-D duplicates selected blocks.
- Two-finger trackpad scrolling pans; pinching zooms. With a mouse, hold Space and drag to pan, or Command/Ctrl-scroll to zoom.
- Click the zoom percentage to fit the workflow into view.
- Right-click the canvas to add a block.

## Storage and current limits

Projects save automatically in the OS application-data folder. Fresh macOS installs use `~/Library/Application Support/dev.Skarva.Skarva/projects`. Generated files and plain JSON context records are stored beside the projects folder. Existing installs continue using their original data directory and vault key, preserving projects, attachments, remembered context, and integrations through the rebrand.

Gmail workflows currently handle one recent unread message per run, checking every 15 seconds while waiting. Processed message IDs are tracked for the current app session. Each run keeps one generated attachment. Office generation supports basic text, tables, and slides; charts, formulas, images, and templates are not included.

Teams, Outlook, and Copilot are not connected yet and are hidden from the block library. Old projects containing these blocks remain readable, and Play explains which block needs replacing.

Icons use Material Symbols Rounded; see [third-party notices](THIRD_PARTY_NOTICES.md). No license has been selected for the original source code.
