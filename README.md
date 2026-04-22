[English](README.md) | [日本語](README.ja.md)

# remotty

![remotty: Windows bridge for Codex and Telegram](docs/assets/hero.png)

`remotty` lets you continue local Codex work from Telegram.

You send a message to your Telegram bot. `remotty` receives it on your Windows
PC, sends it to the selected Codex thread, and returns the reply to the same
Telegram chat.

`remotty` does not expose a public webhook server. It also does not type into
the open Codex App window. It talks to local Codex through the local `codex`
command.

> [!WARNING]
> **Disclaimer**
>
> This is an unofficial community project. It is not affiliated with,
> endorsed by, or sponsored by OpenAI.
> `Codex`, `ChatGPT`, and related marks are trademarks of OpenAI.
> They are referenced here only to describe the local tools that `remotty`
> works with. All other trademarks belong to their owners.

## What It Does

- Connects a Telegram bot to Codex on your Windows PC.
- Lets a Telegram chat choose the Codex thread to continue.
- Sends Telegram messages to that thread.
- Returns Codex replies to the same Telegram chat.
- Relays approval prompts to Telegram.
- Stores the bot token in Windows protected storage.
- Stores `remotty` state under `%APPDATA%\remotty`.

## When To Use It

Use `remotty` when you want to leave your desk and keep steering the Codex work
that is available on your Windows PC.

Use Codex Remote connections when the project itself lives on an SSH machine.
That feature connects the Codex App to a remote host. `remotty` is for reaching
your local Windows Codex setup from Telegram.

## Requirements

- Windows 10 or Windows 11
- Codex App and Codex CLI
- Node.js and `npm`
- A Telegram bot token from `@BotFather`

Rust is only needed when you build from source.

## Get Started

Use the [Telegram Quickstart](docs/telegram-quickstart.md).

It walks through installation, bot setup, token storage, pairing, thread
selection, and a first Telegram test message.

Want to try the local loop before making a Telegram bot?
Use the [Fakechat Demo](docs/fakechat-demo.md).

## Main Commands

Run these in the Codex App after installing the local `remotty` plugin:

```text
/remotty-configure
/remotty-start
/remotty-access-pair <code>
/remotty-policy-allowlist
/remotty-status
/remotty-live-env-check
/remotty-sessions
/remotty-sessions <thread_id>
```

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

- Use `/remotty-configure` so bot tokens stay in Windows protected storage.
- Use a dedicated Telegram bot for `remotty`.
- Do not paste bot tokens or `api.telegram.org/bot...` URLs into issues.
- Keep project files separate from `%APPDATA%\remotty` runtime state.

## Related Docs

- [Telegram Quickstart](docs/telegram-quickstart.md)
- [Fakechat Demo](docs/fakechat-demo.md)
- [Advanced CLI Mode](docs/exec-transport.md)
- [Upgrade Notes](docs/upgrading.md)
- [Development](docs/development.md)

## License

[MIT](LICENSE)
