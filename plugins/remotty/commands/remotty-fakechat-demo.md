# /remotty-fakechat-demo

Start the local fakechat demo or explain how to run it.

Use this when the user wants to try `remotty` before setting up Telegram.

Steps:

1. Explain that fakechat is local-only and does not need a Telegram bot token.
2. Ask the user which workspace folder they want Codex to inspect.
3. Run `remotty demo fakechat --workspace <path>`.
4. Tell the user to open the printed `http://127.0.0.1:<port>` URL.

If port `8787` is busy, retry with:

```powershell
remotty demo fakechat --port 8790 --workspace <path>
```

Do not ask for or print any Telegram token.
