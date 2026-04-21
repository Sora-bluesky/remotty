# Fakechat Demo

Use `remotty demo fakechat` when you want to try the local chat loop before creating a Telegram bot.

The demo starts a browser chat UI on `localhost`, sends each message to the local `codex` CLI, and shows the reply in the same page. It does not use Telegram, bot tokens, webhooks, DPAPI, or a public server.

## What You Need

- Windows 10 or Windows 11
- `remotty`
- `codex` CLI available on `PATH`

## Start the Demo

From the folder you want Codex to inspect, run:

```powershell
remotty demo fakechat
```

Open the printed URL:

```text
http://127.0.0.1:8787
```

Send a small request:

```text
What files are in this workspace?
```

`remotty` runs `codex exec` locally in read-only mode and returns the reply to the page.

## Options

Use a different port:

```powershell
remotty demo fakechat --port 8790
```

Use a specific workspace:

```powershell
remotty demo fakechat --workspace C:\Users\you\Documents\project
```

Use a specific Codex binary or model:

```powershell
remotty demo fakechat --codex-binary codex --model gpt-5.4-mini
```

## How This Differs From Telegram

Fakechat is only a local demo. It proves that `remotty` can call your local Codex setup and return a chat reply.

Telegram mode adds the real remote surface:

- bot setup through `@BotFather`
- account pairing and allowlists
- approval messages in Telegram
- use from your phone while the bridge is running

After the demo works, continue with the [Telegram Quickstart](telegram-quickstart.md).
