use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;

use anyhow::{Result, anyhow};
use windows::Win32::Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE};
use windows::Win32::System::Threading::{CreateMutexW, ReleaseMutex};
use windows::core::PCWSTR;

pub struct TelegramPollerGuard {
    handle: HANDLE,
}

impl TelegramPollerGuard {
    pub fn acquire(bot_id: i64) -> Result<Self> {
        let name = mutex_name(bot_id);
        let encoded_name = encode_wide(&name);
        let handle = unsafe { CreateMutexW(None, true, PCWSTR(encoded_name.as_ptr())) }
            .map_err(|error| anyhow!("telegram poller guard を作れませんでした: {error}"))?;

        let already_exists = unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;
        if already_exists {
            unsafe {
                let _ = CloseHandle(handle);
            }
            return Err(anyhow!(
                "同じ bot を読む別の bridge がすでに動いています。別の `remotty` を止めてから再実行してください。\n{}",
                polling_conflict_hint()
            ));
        }

        Ok(Self { handle })
    }
}

fn polling_conflict_hint() -> &'static str {
    "Windows では `Get-Process remotty, codex -ErrorAction SilentlyContinue | Select-Object Id,ProcessName,Path` で候補を確認できます。対象が分かる場合は `Stop-Process -Id <PID>` で止めてください。"
}

impl Drop for TelegramPollerGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = ReleaseMutex(self.handle);
            let _ = CloseHandle(self.handle);
        }
    }
}

fn mutex_name(bot_id: i64) -> String {
    format!(r"Global\remotty_telegram_bot_{bot_id}")
}

fn encode_wide(value: &str) -> Vec<u16> {
    OsStr::new(value)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{TelegramPollerGuard, mutex_name};
    use uuid::Uuid;

    #[test]
    fn mutex_name_uses_bot_id() {
        assert_eq!(
            mutex_name(8642321094),
            r"Global\remotty_telegram_bot_8642321094"
        );
    }

    #[test]
    fn acquiring_same_bot_guard_twice_fails_until_first_guard_drops() {
        let bot_id = i64::from_le_bytes(*Uuid::new_v4().as_bytes().first_chunk().unwrap()).abs();
        let first_guard = TelegramPollerGuard::acquire(bot_id).expect("first guard should acquire");

        let second_attempt = TelegramPollerGuard::acquire(bot_id);
        assert!(second_attempt.is_err());
        match second_attempt {
            Ok(_) => panic!("second guard should fail"),
            Err(error) => assert!(error.to_string().contains("Get-Process")),
        }

        drop(first_guard);

        TelegramPollerGuard::acquire(bot_id).expect("guard should reacquire after drop");
    }
}
