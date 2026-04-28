[English](README.md) | [日本語](README.ja.md)

# remotty

![remotty: Windows bridge for Codex and Telegram](docs/assets/hero.png)

`remotty` is not a general-purpose remote control tool.
It is a bridge for continuing Codex work on Windows from the Telegram app you
already use.

`remotty` lets you continue Codex work from Telegram.
There is no new mobile app to install.

You send a message to your Telegram bot. `remotty` receives it on your Windows
PC, sends it to the selected Codex thread, and returns the reply to the same
Telegram chat.

`remotty` does not expose a public webhook server. It also does not type into
the open Codex App window. It talks to local Codex through the local `codex`
command.

## What It Does

- Connects a Telegram bot to Codex on your Windows PC.
- Lets a Telegram chat choose the Codex thread to continue.
- Sends Telegram messages to that thread.
- Queues text you send while Codex is already working.
- Returns Codex replies to the same Telegram chat.
- Relays approval prompts to Telegram.
- Stores the bot token in Windows protected storage.
- Stores `remotty` state under `%APPDATA%\remotty`.

## When To Use It

Use `remotty` when you want to leave your desk and keep steering the Codex work
that is available on your Windows PC.

## Requirements

- Windows 10 or Windows 11
- Codex CLI
- Node.js and `npm`
- A Telegram bot token from `@BotFather`

Rust is only needed when you build from source.

## Get Started

Use the [Telegram Quickstart](docs/telegram-quickstart.md).

It walks through installation, bot setup, token storage, pairing, thread
selection, and a first Telegram test message.

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

## Main Commands

Run these from PowerShell:

```powershell
$configPath = Join-Path $env:APPDATA "remotty\bridge.toml"
remotty config workspace upsert --config $configPath --path (Get-Location).Path
remotty telegram configure --config $configPath
remotty --config $configPath
remotty telegram access-pair <code> --config $configPath
remotty telegram policy allowlist --config $configPath
remotty telegram sessions --config $configPath
```

When `remotty --config $configPath` succeeds, the terminal prints
`Listening for Telegram channel messages from: remotty:telegram`.
Keep that terminal open while you use Telegram.

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
/remotty-sessions <thread title or ID>
/workspace
/workspace <id>
```

Thread titles may include spaces.
No quotes are needed.
If more than one thread matches, use the shown `ID`.
If a title also looks like another thread's `ID`, use the shown `ID`.

## Security

- Store bot tokens with `remotty telegram configure` so they stay in Windows protected storage.
- Use a dedicated Telegram bot for `remotty`.
- Do not paste bot tokens into chat, issues, or pull requests.
- Do not paste bot tokens or `api.telegram.org/bot...` URLs into issues.
- Keep project files separate from `%APPDATA%\remotty` runtime state.

## Related Docs

- [Telegram Quickstart](docs/telegram-quickstart.md)
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
