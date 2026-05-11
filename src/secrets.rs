#![cfg(feature = "quota-sync")]

use color_eyre::{eyre::eyre, Result};

const SERVICE: &str = "dev.tokenuse";

pub fn store(account: &str, value: &str) -> Result<()> {
    let entry = keyring::Entry::new(SERVICE, account)
        .map_err(|e| eyre!("open keychain entry for {account}: {e}"))?;
    entry
        .set_password(value)
        .map_err(|e| eyre!("write keychain entry for {account}: {e}"))?;
    Ok(())
}

pub fn read(account: &str) -> Result<Option<String>> {
    let entry = keyring::Entry::new(SERVICE, account)
        .map_err(|e| eyre!("open keychain entry for {account}: {e}"))?;
    match entry.get_password() {
        Ok(value) => Ok(Some(value)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(eyre!("read keychain entry for {account}: {e}")),
    }
}

pub fn delete(account: &str) -> Result<()> {
    let entry = keyring::Entry::new(SERVICE, account)
        .map_err(|e| eyre!("open keychain entry for {account}: {e}"))?;
    match entry.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(eyre!("delete keychain entry for {account}: {e}")),
    }
}
