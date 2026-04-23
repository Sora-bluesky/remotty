# Telegram Quickstart

This guide sets up `remotty` so Telegram can send messages to a Codex thread
on your Windows PC.

## How It Works

1. You run `remotty` on your Windows PC.
2. You send a message to your Telegram bot.
3. `remotty` sends that message to the Codex thread you selected.
4. Codex replies, and `remotty` sends the reply back to Telegram.

## What You Need

- Windows 10 or Windows 11
- Codex App and Codex CLI
- Node.js and `npm`
- Telegram
- A dedicated Telegram bot from `@BotFather`

## 1. Install `remotty`

Run this in PowerShell:

```powershell
npm install -g remotty
```

Save the config path in a variable:

```powershell
$configPath = Join-Path $env:APPDATA "remotty\bridge.toml"
```

The PowerShell examples below reuse `$configPath`.

## Where To Type Commands And Where Data Is Stored

In Codex App, type `@` in the chat box.
Select `remotty` from the suggestions.
Then describe the setup task in plain language.
Do not type those plugin requests in PowerShell.
This guide explicitly says when a command goes to Telegram.

The bot token is not saved in your project repository.
It is saved in Windows protected storage.
The protected file lives under `%LOCALAPPDATA%\remotty\secrets`.
By default, the file name is `remotty-telegram-bot.bin`.
`remotty` config and runtime state are saved under `%APPDATA%\remotty`.

Register the project while the target project is open.
Token storage and bridge startup do not write to the repository.
For the clearest setup, keep using the same project while you run them.
`remotty` does not create files in the project root.
In normal use, it does not add anything to commit.

## How Often Each Step Is Needed

Do these once for the same Windows user:

- Install `remotty`
- Install the local plugin in Codex App
- Prepare a Telegram bot, unless you already have one
- Store the bot token
- Pair your Telegram account

Do these when you use a new project:

- Open or enter the project
- Register that project with `remotty`

Do this for each Telegram chat:

- Bind a Telegram chat to a Codex thread

Check these when you use it:

- Confirm the bridge is running
- Confirm the Telegram chat points to the intended Codex thread
- Send a message from Telegram

## 2. Install the Local Plugin (Once)

Codex App users can use the local plugin.

In the Codex App Plugins view:

1. Select `remotty local plugins` in the plugin source selector.
2. Click the add button on the `remotty` plugin.
3. Confirm the install dialog.

After selecting `remotty local plugins`, `remotty` appears in the plugin list.

![Codex plugin list with remotty local plugins selected](assets/quickstart/codex-plugin-marketplace-select.png)

Click the add button, then confirm the install dialog.

![Codex plugin install dialog for remotty](assets/quickstart/codex-plugin-install-remotty.png)

Codex CLI users can skip this step.
Use the PowerShell commands shown below instead.

## 3. Open or Enter Your Project (When You Use a New Project)

Use the project you want to continue from Telegram.
You do not need to use the same project every time.

If you use Codex App, open that project in the app.

If you use Codex CLI, enter the project folder in PowerShell:

```powershell
Set-Location C:\path\to\your\project
```

## 4. Register This Project (Once Per Project)

In Codex App, use a normal chat request while the target project is open.
This requires the `remotty` plugin to be installed.

```text
Register this project with remotty
```

If the plugin does not respond, use the PowerShell command below.

Codex CLI users run this from the project folder:

```powershell
remotty config workspace upsert --config $configPath --path (Get-Location).Path
```

This saves the project to the config under `%APPDATA%\remotty`.
It does not write files into your project repository.
It also does not create `.remotty` or other files in the project root.
If you want to verify that, run `git status`.

## 5. Prepare a Telegram Bot (Once)

This is a one-time setup.
If you already have a dedicated `remotty` bot, use its token.

Only create a new bot when you do not have one yet:

1. Open `@BotFather` in Telegram.
2. Send `/newbot`.
3. Choose a display name.
4. Choose a username ending in `bot`.
5. Copy the token that BotFather returns.

Do not post the token in chat, screenshots, issues, or pull requests.

## 6. Store the Bot Token (Once, Or When Replacing It)

In Codex App, ask:

```text
Store the Telegram bot token
```

This does not save the token in the open repository.
After you store it once, the same Windows user can reuse it.
Run this again only when you want to replace the token.

`remotty` opens a PowerShell window for hidden token input.
Enter the token only in that PowerShell window.
Do not paste the token into Codex App chat.

Codex CLI users run:

```powershell
remotty telegram configure --config $configPath
```

Paste the token when prompted.
The command stores it in Windows protected storage.
It does not print the token back.
The encrypted file is under `%LOCALAPPDATA%\remotty\secrets`.
The default file name is `remotty-telegram-bot.bin`.
The storage is tied to your Windows user.
It is reused even when you work in another project.

## 7. Start the Bridge (When You Use It)

In Codex App, ask:

```text
Start the bridge
```

Startup uses `%APPDATA%\remotty\bridge.toml`.
It does not put runtime files in the open repository:

Codex CLI users run:

```powershell
# Start the foreground bridge.
remotty --config $configPath
```

Keep the bridge running while you use Telegram.
If it stops, the bot cannot reply.

## 8. Pair Telegram (Once)

Send any message to your bot in a private Telegram chat.

The bot replies with a `remotty pairing code`.
In Codex App, ask:

```text
Pair with code <code>
```

Codex CLI users run:

```powershell
remotty telegram access-pair <code> --config $configPath
```

Then check the allowlist.
In Codex App, ask:

```text
Lock down Telegram access to the allowlist
```

Codex CLI users run:

```powershell
remotty telegram policy allowlist --config $configPath
```

This prevents other Telegram users from controlling your local Codex setup.

## 9. Select a Codex Thread (Per Telegram Chat)

In Codex App, ask:

```text
List Codex threads
```

Codex CLI users run:

```powershell
remotty telegram sessions --config $configPath
```

Choose the thread you want Telegram to continue.
Then send this in the target Telegram chat:

For example, if the thread title is `Start workspace session`:

```text
/remotty-sessions Start workspace session
```

Everything after `/remotty-sessions` is treated as one title.
No quotes are needed.
Matching is case-insensitive.
`remotty` tries exact `ID`, exact title, `ID` prefix, then a title substring match.
If more than one thread matches, use the shown `ID`.
If a title also looks like another thread's `ID`, `remotty` asks you to choose by `ID`.

This binding is stored under `%APPDATA%\remotty`.
It is not written into your project repository.

## 10. Send a Test Message

In Telegram, send:

```text
Summarize the current thread and suggest the next step.
```

`remotty` sends the text to the selected Codex thread.
The reply appears in Telegram.

## Approval Prompts

When Codex asks for approval, `remotty` posts the prompt to Telegram.

You can press `Approve` or `Deny`.
You can also type:

```text
/approve <request_id>
/deny <request_id>
```

The decision is returned to the same Codex turn.

## Q&A

### Security Q&A

> Q. Does project registration create files in my project?
>
> A. No. It saves configuration to `%APPDATA%\remotty\bridge.toml`. It does not create `.remotty` or other files in the project root. If you want to verify this, run `git status` after the command.

> Q. Where is the bot token stored?
>
> A. It is stored in Windows protected storage under `%LOCALAPPDATA%\remotty\secrets`. The default file name is `remotty-telegram-bot.bin`. It is not stored in your project repository, GitHub, or a Telegram chat.

> Q. Is the bot token sent to OpenAI or another public server?
>
> A. `remotty` uses the token to connect to the Telegram API. It does not need to send the token to OpenAI. Do not paste the token into issues, pull requests, or screenshots.

> Q. Does `remotty` expose a public webhook server?
>
> A. No. The normal setup polls Telegram from your Windows PC. You do not need to open a router port.

> Q. Can anyone control my Codex session?
>
> A. No. Only paired senders are allowed. After pairing, ask the `remotty` plugin to lock down access to the allowlist.
> This keeps access limited to configured senders.

> Q. Is approving from Telegram safe?
>
> A. Only allowed senders can approve. Approval still continues local Codex work, so allow only Telegram accounts you trust.

> Q. Can I use the same bot across projects?
>
> A. Yes. The bot token is stored for your Windows user. Project registration and Telegram chat bindings are separate.

> Q. What if the token may have leaked?
>
> A. Regenerate it with `@BotFather`. Then ask the `remotty` plugin to save the new token.

### Connection Q&A

> Q. The `remotty` plugin is installed, but `@remotty` does not appear in chat.
>
> A. Keep the current chat open.
> You do not need an `@remotty` mention.
> Try the same request as normal chat text.
> If that does not trigger the plugin, use PowerShell.
> Move to the project folder first:
>
> ```powershell
> Set-Location C:\path\to\your-project
> ```
>
> Then run:
>
> ```powershell
> remotty config workspace upsert --config $configPath --path (Get-Location).Path
> ```
>
> This registers the project without creating files in the project root.
> Then continue the remaining setup from PowerShell.
> If the plugin starts responding later in Codex App, you can return there.

> Q. The bot does not reply.
>
> A. First confirm the bridge is still running. In Codex App, ask the `remotty` plugin to check status. In PowerShell, run `remotty service status` and `remotty telegram live-env-check --config $configPath`. If the webhook status is `webhook-configured`, switch the bot back to polling.

> Q. I get a polling conflict.
>
> A. Only one process can poll the same Telegram bot. On Windows, list likely processes:

```powershell
Get-Process remotty, codex -ErrorAction SilentlyContinue | Select-Object Id,ProcessName,Path
```

> Stop the process that reads the same bot:

```powershell
Stop-Process -Id <PID>
```

### Pairing Q&A

> Q. The pairing code does not work.
>
> A. Send the message in a private chat with the bot. Use the newest code. Ask the `remotty` plugin to pair before the code expires.

### Thread Selection Q&A

> Q. No Codex threads appear.
>
> A. Update Codex CLI, then try again. Start at least one Codex App or Codex CLI thread. Ask the `remotty` plugin to list threads again.

## Related

- [Fakechat Demo](fakechat-demo.md)
- [Advanced CLI Mode](exec-transport.md)
- [Upgrade Notes](upgrading.md)

Note: if your code and shell live on an SSH host, Codex Remote connections may
also be useful. `remotty` is for returning to Codex work on your Windows PC
from Telegram.
