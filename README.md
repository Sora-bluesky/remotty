[English](README.md) | [日本語](README.ja.md)

# remotty

![remotty: Telegram bridge for Codex on Windows](docs/assets/hero.png)

`remotty` is a Telegram bridge for watching Codex work and sending short follow-ups on Windows.

The common failure mode for delegated AI work is simple: you walk away, and the
run stops on an approval prompt, an error, or a small missing instruction.
`remotty` gives you a narrow Telegram surface for that moment.

Use Codex CLI as the local session that `remotty` connects to today.
Use the main Codex workspace, or Codex CLI, for richer task control, diff
review, and long instructions.
Use `remotty` for Telegram notifications, approval relay, concise status, and
short follow-up messages while away from the full Codex interface.

Here, "main Codex workspace" does not mean a specific mobile app.
It means the place where you read work, review diffs, and give fuller instructions.
`remotty` does not replace that workspace.

You send a message to your Telegram bot.
`remotty` receives it on your Windows PC, sends it to the Codex CLI session you connected, and returns the reply to the same Telegram chat.

`remotty` does not expose a public webhook server. It also does not type into
the open Codex App window.
It is not an official remote-control surface for the Codex App.
In the current public flow, it talks to local Codex through the local `codex`
command and the `app_server` transport.

## What It Is Not

`remotty` is not a mobile Codex App or a Web IDE.
Its strength is letting you answer the small moments that matter while away
from your desk: notifications, approvals, short replies, and Codex input
requests through Telegram.
It is a lightweight contact point for the moments when Codex needs a human,
not a full remote workspace.

The positioning is:

- [Codex Remote connections](https://developers.openai.com/codex/remote-connections):
  an OpenAI feature that lets Codex work in projects on SSH-connected or cloud
  environments.
- A mobile web app: a full browser UI that a developer may build separately to
  operate Codex remotely.
- `remotty`: a lightweight Telegram contact point for Codex confirmations,
  approvals, and input requests.

## What It Does

- Connects a Telegram bot to Codex on your Windows PC.
- Connects Telegram to the Codex CLI session you start for the project.
- Sends Telegram messages to that session.
- Queues text you send while Codex is already working.
- Returns Codex replies to the same Telegram chat.
- Relays approval prompts to Telegram.
- Stores the bot token in Windows protected storage.
- Stores `remotty` state under `%APPDATA%\remotty`.

## When To Use It

Use `remotty` when you want to leave your desk and keep watching, approving, or
sending short follow-ups to Codex work that is running on your Windows PC.

## Requirements

- Windows 10 or Windows 11
- Codex CLI
- Node.js and `npm`
- A Telegram bot token from `@BotFather`

Rust is only needed when you build from source.

## Get Started

Use the [Telegram Quickstart](docs/telegram-quickstart.md).

It walks through installation, bot setup, starting `remotty remote-control`,
pairing, and a first Telegram test message.

Want to try the local loop before making a Telegram bot?
Use the [Fakechat Demo](docs/fakechat-demo.md).

## Update

Run this in a normal user PowerShell when you want the latest published
`remotty` package:

```powershell
npm install -g remotty
```

Then follow the [Telegram Quickstart](docs/telegram-quickstart.md) from the
project you want to use.

## Quickstart Overview

The shortest path is to start Codex CLI for a project, then run
`remotty remote-control` for the same project in another PowerShell window.
By default, if `%APPDATA%\remotty\bridge.toml` is missing, `remotty` creates it.
It registers the current project on every run, and asks for your Telegram bot
token only when none is already stored.
Advanced users can pass an explicit config path in the quickstart commands.

| Window | Keep it open? | Use it for |
| --- | --- | --- |
| Normal PowerShell | No | Install `remotty` and finish Telegram pairing when needed. |
| Codex PowerShell | Yes | Start `codex` in the project you want to continue from Telegram. |
| Remote Control PowerShell | Yes | Run `remotty remote-control` in the same project. |

Follow the [Telegram Quickstart](docs/telegram-quickstart.md) for the exact
step-by-step commands.

When remote control starts successfully, the terminal prints
`Remote Control active` and
`Listening for Telegram channel messages from: remotty:telegram`.
Keep that Remote Control PowerShell window open while you use Telegram.

If you also use Codex App, the bundled plugin can help with setup tasks.
The plugin is optional. The supported Telegram flow is the Codex CLI flow above:
Codex CLI plus the `remotty` PowerShell commands.

Common Telegram commands are:

```text
/help
/status
/stop
/approve <request_id>
/deny <request_id>
/answer <request_id> <value>
/workspace
/workspace <id>
```

Send `/help` to the bot for the full command list, including session and mode
controls.

For `/answer` details, see [Codex Input Requests](docs/telegram-quickstart.md#codex-input-requests).

## Security

- Store bot tokens with `remotty remote-control` or `remotty telegram configure` so they stay in Windows protected storage.
- Use a dedicated Telegram bot for `remotty`.
- Do not paste bot tokens into chat, issues, or pull requests.
- Do not paste bot tokens or `api.telegram.org/bot...` URLs into issues.
- Use the same Windows user to store the token and run `remotty`.
- `remotty` rejects Codex input requests marked as secret. Treat Telegram
  messages as chat history: do not send passwords, API keys, recovery codes, or
  other secrets with `/answer`.
- Keep project files separate from `%APPDATA%\remotty` runtime state.

## Related Docs

- [Telegram Quickstart](docs/telegram-quickstart.md)
- [Telegram Bridge Direction](docs/remote-companion.md)
- [Fakechat Demo](docs/fakechat-demo.md)
- [Advanced CLI Mode](docs/exec-transport.md)
- [Upgrade Notes](docs/upgrading.md)

Note: if your project lives on an SSH host, Codex Remote connections may also
be useful. `remotty` is for returning to Codex work on your Windows PC from
Telegram.

## License

[MIT](LICENSE)

## Disclaimer

This is an unofficial community project. It is not affiliated with, endorsed by,
or sponsored by OpenAI.

`Codex`, `ChatGPT`, and related marks are trademarks of OpenAI.
They are referenced here only to describe the local tools that `remotty` works
with. All other trademarks belong to their owners.
