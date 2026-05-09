[English](README.md) | [日本語](README.ja.md)

# remotty

![remotty: Telegram bridge for Codex on Windows](docs/assets/hero.png)

`remotty` is not a replacement for the Codex App.
It is a Telegram bridge for watching and lightly steering Codex work on Windows.

The common failure mode for delegated AI work is simple: you walk away, and the
run stops on an approval prompt, an error, or a small missing instruction.
`remotty` gives you a narrow Telegram surface for that moment.

Use Codex CLI as the local session that `remotty` connects to today.
Use the Codex App or Codex CLI for richer task control, diff review, and long
instructions.
Use `remotty` for Telegram notifications, approval relay, concise status, and
short follow-up messages while away from the full Codex interface.

You send a message to your Telegram bot.
`remotty` receives it on your Windows PC, sends it to the Codex CLI session you connected, and returns the reply to the same Telegram chat.

`remotty` does not expose a public webhook server. It also does not type into
the open Codex App window.
It is not an official remote-control surface for Codex.
In the current public flow, it talks to local Codex through the local `codex`
command and the `app_server` transport.

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
lightly steering Codex work that is running on your Windows PC.

## Requirements

- Windows 10 or Windows 11
- Codex CLI
- Node.js and `npm`
- A Telegram bot token from `@BotFather`

Rust is only needed when you build from source.

## Get Started

Use the [Telegram Quickstart](docs/telegram-quickstart.md).

It walks through installation, bot setup, token storage, pairing, channel
startup, and a first Telegram test message.

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

Do not copy the setup commands as one script.
The Telegram setup uses separate PowerShell windows because the `codex` and
`remotty` processes both stay open while you use Telegram.

| Window | Keep it open? | Use it for |
| --- | --- | --- |
| Setup PowerShell | No | Install `remotty`, register the project, store the bot token, and pair Telegram. |
| Codex PowerShell | Yes | Start `codex` in the project you want to continue from Telegram. |
| Bridge PowerShell | Yes | Start `remotty --config "$env:APPDATA\remotty\bridge.toml"` for the same project. |

Follow the [Telegram Quickstart](docs/telegram-quickstart.md) for the exact
step-by-step commands.

When the bridge starts successfully, the terminal prints
`Listening for Telegram channel messages from: remotty:telegram`.
Keep that Bridge PowerShell window open while you use Telegram.

If you also use Codex App, the bundled plugin can help with setup tasks.
The plugin is optional. The supported Telegram flow is the Codex CLI flow above:
Codex CLI plus the `remotty` PowerShell commands.

Run these in Telegram:

```text
/help
/status
/stop
/approve <request_id>
/deny <request_id>
/workspace
/workspace <id>
```

## Security

- Store bot tokens with `remotty telegram configure` so they stay in Windows protected storage.
- Use a dedicated Telegram bot for `remotty`.
- Do not paste bot tokens into chat, issues, or pull requests.
- Do not paste bot tokens or `api.telegram.org/bot...` URLs into issues.
- Keep project files separate from `%APPDATA%\remotty` runtime state.

## Related Docs

- [Telegram Quickstart](docs/telegram-quickstart.md)
- [Remote Bridge Direction](docs/remote-companion.md)
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
