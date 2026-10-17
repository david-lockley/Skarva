# JEV Studio

A native Rust prototype for building agent workflows on a visual canvas. The editor supports both local preview and Jev-powered routing between connected branches. App integrations such as Teams, Outlook, and Office remain visual placeholders.

## Run

Install Rust, then run `cargo run --release` from this folder. This uses a native GPU renderer (Metal on macOS, DirectX 12 or Vulkan on Windows); it does not use a webview.

On macOS, `sh scripts/package-macos.sh release` also creates `target/release/JEV Studio.app`. Open that bundle from Finder or with `open "target/release/JEV Studio.app"`. On Windows, build and run with `cargo run --release` in PowerShell; Windows packaging is not included yet.

Projects save automatically as JSON in the operating system's application-data folder. On macOS this is `~/Library/Application Support/dev.JEV-Studio.JEV-Studio/projects`.

## Controls

- Expand Flow & canvas, Messages, AI & assistants, and Office apps in the left panel, then click a block to add it. Multiple sections can be open at once. Search finds blocks across all categories. Blocks are initially arranged in a loose grid and can then be freely positioned.
- Drag blocks to move them. Drag from an output port to another block (or its input port) to connect them; click-then-click also works. Drop a connection on empty canvas to create and connect a new block.
- Connect multiple blocks directly to one output, without adding a Decision block. The inspector lets you choose which branch Play simulates. The explicit Decision block is still available.
- Click a block to edit its name and prototype settings in the right panel. Click a connection line to select it, then press Delete/Backspace or use Remove connection in the inspector.
- On a trackpad, slide two fingers to pan and pinch to zoom. Hold Space and drag to pan with a mouse, or hold Command (macOS) / Ctrl (Windows) and scroll to zoom. Plain background dragging does nothing. Use the zoom control to reset the view.
- Right-click the canvas for the same categorized block picker. Dropping a connection on empty canvas opens it with only connectable blocks shown.
- Select a Start block and press Play to follow that path. If no Start block is selected, Play uses the first one on the canvas. Pause preserves a run in progress; Reset clears the run log and highlighted path.

## Jev live routing

1. In a project, click **Jev settings** in the top-right toolbar. Paste a TypeSafe API key from the [official console](https://console.typesafe.ai/) and click **Save key**. The key is stored in macOS Keychain or Windows Credential Manager, not in project JSON. You can remove it from the same dialog. Do not paste a key into a block title, subtitle, test message, or project file.
2. Choose **Jev live** from the toolbar's run-mode dropdown. Expand the Run Log and enter a sample **Test message**, then press Play.
3. At any block with more than one connected outgoing path, Jev receives the test message, current block, path so far, and descriptions of the connected destinations. It returns a Choice, probability, and confidence. If confidence is below that block's gate (70% by default), the run waits for you to choose a path. The gate is editable in the block inspector.

Jev live mode sends the sample message, visited block names, and connected block titles/subtitles to TypeSafe's API. It does not read real Teams/Outlook messages or execute the destination integrations. Preview mode makes no API calls. The sample message is transient and is not saved with the project.

Each project can contain multiple canvases, and each canvas can have multiple Start blocks. Preview mode visits connected blocks and uses the configured mock route on Decision blocks or the selected mock target for ordinary branching. Teams, Outlook, Copilot, OpenAI, Word, Excel, and PowerPoint are visual placeholders for future integrations.

Interface icons come from Material Symbols Rounded; see [third-party notices](THIRD_PARTY_NOTICES.md). No license has been selected yet for JEV Studio's original source code.
