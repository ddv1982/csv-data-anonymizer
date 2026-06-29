use std::io;

#[cfg(not(test))]
const SEED_VAULT_SERVICE: &str = "csv-data-anonymizer";
#[cfg(not(test))]
const SEED_VAULT_ACCOUNT: &str = "repeatable-replacement-seed";

pub trait SeedVault: Send + Sync {
    fn load_seed(&self) -> io::Result<Option<String>>;
    fn save_seed(&self, seed: &str) -> io::Result<()>;
    fn delete_seed(&self) -> io::Result<()>;
}

#[derive(Debug, Default)]
#[cfg(not(test))]
pub struct KeyringSeedVault;

#[cfg(not(test))]
impl SeedVault for KeyringSeedVault {
    fn load_seed(&self) -> io::Result<Option<String>> {
        let entry = seed_entry()?;
        match entry.get_password() {
            Ok(seed) => Ok(Some(seed)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(keyring_error("load private seed", error)),
        }
    }

    fn save_seed(&self, seed: &str) -> io::Result<()> {
        seed_entry()?
            .set_password(seed)
            .map_err(|error| keyring_error("save private seed", error))
    }

    fn delete_seed(&self) -> io::Result<()> {
        let entry = seed_entry()?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(keyring_error("delete private seed", error)),
        }
    }
}

#[cfg(not(test))]
fn seed_entry() -> io::Result<keyring::Entry> {
    keyring::Entry::new(SEED_VAULT_SERVICE, SEED_VAULT_ACCOUNT)
        .map_err(|error| keyring_error("open private seed vault", error))
}

#[cfg(not(test))]
fn keyring_error(action: &str, error: keyring::Error) -> io::Error {
    io::Error::other(format!("Could not {action}: {error}"))
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use std::sync::Mutex;

    #[derive(Debug, Default)]
    pub struct MemorySeedVault {
        seed: Mutex<Option<String>>,
        fail_save: bool,
        fail_load: bool,
        fail_delete: bool,
    }

    impl MemorySeedVault {
        pub fn with_save_failure() -> Self {
            Self {
                seed: Mutex::new(None),
                fail_save: true,
                fail_load: false,
                fail_delete: false,
            }
        }

        pub fn with_delete_failure(seed: impl Into<String>) -> Self {
            Self {
                seed: Mutex::new(Some(seed.into())),
                fail_save: false,
                fail_load: false,
                fail_delete: true,
            }
        }

        pub fn seed(&self) -> Option<String> {
            self.seed.lock().unwrap().clone()
        }
    }

    impl SeedVault for MemorySeedVault {
        fn load_seed(&self) -> io::Result<Option<String>> {
            if self.fail_load {
                return Err(io::Error::other("seed vault load failed"));
            }
            Ok(self.seed.lock().unwrap().clone())
        }

        fn save_seed(&self, seed: &str) -> io::Result<()> {
            if self.fail_save {
                return Err(io::Error::other("seed vault save failed"));
            }
            *self.seed.lock().unwrap() = Some(seed.to_string());
            Ok(())
        }

        fn delete_seed(&self) -> io::Result<()> {
            if self.fail_delete {
                return Err(io::Error::other("seed vault delete failed"));
            }
            *self.seed.lock().unwrap() = None;
            Ok(())
        }
    }
}
