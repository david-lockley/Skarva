# Connect your services

Start small: a Text workflow needs just one AI connection. Jev is needed only when a block has multiple outgoing paths; Tavily and Gmail are optional.

## AI model

Open **Integrations → AI model**, select OpenRouter, OpenAI, or Anthropic, enter a model ID supported by your account, and save your API key. The model field is editable. Defaults are starting points, not a guarantee that a provider currently offers a particular model or free capacity.

API access is billed by the provider, independently of any chat subscription. Key pages: [OpenRouter](https://openrouter.ai/settings/keys), [OpenAI](https://platform.openai.com/api-keys), [Anthropic](https://console.anthropic.com/).

## Jev decisions

Add your [TypeSafe API key](https://console.typesafe.ai/) under **Integrations → Jev decisions**. Connect several destinations to a block. Its routing question and destination descriptions tell Jev what to choose. Use the block's routing settings to set the confidence threshold; uncertain decisions pause for you to pick a path.

Connect both outputs of a Yes / No block. Use End for an answer that should stop the workflow.

## Web research

Create a [Tavily API key](https://app.tavily.com/) and save it under **Web research**. A research block uses the AI connection to form a search query, then Tavily to retrieve sources. Source excerpts and links pass to later steps.

## Gmail

1. Create a project in [Google Cloud Console](https://console.cloud.google.com/) and enable the Gmail API.
2. Configure an External OAuth consent screen in Testing mode. Add your Google account as a test user.
3. Add the `gmail.readonly` and `gmail.send` scopes. Create a **Desktop app** OAuth client.
4. In **Integrations → Gmail → Google OAuth setup**, paste that client's ID and secret and save. Then choose **Connect Gmail** and finish sign-in in your browser.

Use the ID ending in `.apps.googleusercontent.com`, not the project ID or an API key. If Google reports `invalid_client`, confirm that the ID and secret belong to the same Desktop OAuth client. See [Google's desktop OAuth guide](https://developers.google.com/identity/protocols/oauth2/native-app).

Testing-mode refresh tokens may expire after seven days. Reconnect if needed. Distributing an app with shared Gmail OAuth credentials and restricted scopes can require Google's verification; Skarva does not ship a shared client or your credentials.

Create **Email received → AI model → Reply to email → End**, then press Play. Skarva checks for a recent unread inbox message every 15 seconds, handles one message per run, and shows an editable reply before sending. It does not mark the message as read. Processed IDs are remembered for the app session only, so an unread email may be picked up again after restart.

## Local data and privacy

Projects save as JSON in the OS application data directory. On a fresh install:

| Platform | Typical directory |
| --- | --- |
| macOS | `~/Library/Application Support/dev.Skarva.Skarva` |
| Windows | `%LOCALAPPDATA%\Skarva\Skarva\data` |
| Linux | `${XDG_DATA_HOME:-~/.local/share}/skarva` |

Existing installs keep their original directory and vault key so that projects and attachments continue working after the rename. `skarva --data-dir PATH` selects a separate local profile for testing or recovery. Profiles still use the same OS credential service; do not copy an encrypted vault to another computer without its OS master key.

The encrypted vault uses AES-256-GCM. Its random master key stays in macOS Keychain, Windows Credential Manager, or a Linux Secret Service keyring. Skarva unlocks it once per launch and caches both success and failure to avoid repeated password prompts. On Linux, run a Secret Service provider such as GNOME Keyring or KeePassXC with Secret Service integration enabled; no plaintext fallback is used.

Conversation context, project instructions, source files, and generated documents are **not encrypted by Skarva**. Do not commit your application-data directory to Git. Workflow exports omit service keys and conversation history, but include block prompts and configured local file paths. Back up the data directory and use OS disk encryption for sensitive local data.

Data goes to the services your workflow uses: AI providers receive the incoming request and context; Jev receives routing context and connected choices; Tavily receives a search query; Gmail receives an approved reply. Untrusted email and files can influence AI output. Review destinations, text, and attachments before approving a send.

## Current limits

- One recent unread Gmail message per run, searched within the last day. The first ten matching message IDs are considered.
- One generated Office attachment per run; a later Office block replaces the earlier file in the current run.
- Basic DOCX text/tables, XLSX tables, and PPTX slides. Charts, formulas, images, and templates are not generated.
- TXT, Markdown, CSV, TSV, JSON, PDF, DOCX, XLSX, and PPTX input. Up to five email attachments of 5 MB each; local files have the same size limit. Scanned PDFs need external OCR.
- Loops are not supported. Branches choose one destination; they do not run several actions in parallel.
- Context remembers a bounded previous result for the same canvas/conversation, not a full chat archive. Old explicit memory blocks are migrated with a project JSON backup.
- Requests already sent to a provider can finish after Reset; a Gmail send in flight must finish before changing or resetting the run.
