use crate::store::Store;
use crate::{Error, Result};

pub const MAX_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_PLUGIN_BYTES: usize = 5 * 1024 * 1024;
pub const MAX_KEY_CHARS: usize = 128;

fn check_key(key: &str) -> Result<()> {
    let plain = key
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'));
    if key.is_empty() || key.len() > MAX_KEY_CHARS || !plain || key == "." || key == ".." {
        return Err(Error::InvalidInput(format!(
            "a storage key is 1 to {MAX_KEY_CHARS} characters of letters, digits, `.`, `_` and `-`"
        )));
    }
    Ok(())
}

pub async fn get(store: &Store, plugin_id: &str, key: &str) -> Result<String> {
    check_key(key)?;
    store
        .plugin_storage_get(plugin_id, key)
        .await?
        .ok_or_else(|| Error::missing("storage key", key))
}

pub async fn put(store: &Store, plugin_id: &str, key: &str, value: &str) -> Result<()> {
    check_key(key)?;
    if value.len() > MAX_VALUE_BYTES {
        return Err(Error::TooLarge(format!(
            "a storage value is at most {MAX_VALUE_BYTES} bytes, this one is {}",
            value.len()
        )));
    }
    store
        .plugin_storage_put(plugin_id, key, value, MAX_PLUGIN_BYTES)
        .await
}

pub async fn delete(store: &Store, plugin_id: &str, key: &str) -> Result<()> {
    check_key(key)?;
    if store.plugin_storage_delete(plugin_id, key).await? {
        Ok(())
    } else {
        Err(Error::missing("storage key", key))
    }
}
