//! macOS Keychain credential storage.

use tracing::debug;

const SERVICE_NAME: &str = "com.clawx.agent-computer";

/// Keychain credential store for securely storing API keys and tokens.
pub struct KeychainStore;

/// Error from Keychain operations.
#[derive(Debug)]
pub enum KeychainError {
    /// The requested item was not found in the Keychain.
    NotFound(String),
    /// A Keychain operation failed.
    OperationFailed(String),
}

impl std::fmt::Display for KeychainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound(key) => write!(f, "keychain item not found: {}", key),
            Self::OperationFailed(msg) => write!(f, "keychain error: {}", msg),
        }
    }
}

impl std::error::Error for KeychainError {}

impl KeychainStore {
    /// Store a credential in the Keychain.
    pub fn set(account: &str, password: &str) -> Result<(), KeychainError> {
        security_framework::passwords::set_generic_password(
            SERVICE_NAME,
            account,
            password.as_bytes(),
        )
        .map_err(|e| KeychainError::OperationFailed(e.to_string()))?;
        debug!(account, "stored credential in keychain");
        Ok(())
    }

    /// Retrieve a credential from the Keychain.
    pub fn get(account: &str) -> Result<String, KeychainError> {
        let password =
            security_framework::passwords::get_generic_password(SERVICE_NAME, account).map_err(
                |e| {
                    let msg = e.to_string();
                    if msg.contains("not found") || msg.contains("-25300") {
                        KeychainError::NotFound(account.to_string())
                    } else {
                        KeychainError::OperationFailed(msg)
                    }
                },
            )?;
        String::from_utf8(password)
            .map_err(|e| KeychainError::OperationFailed(format!("invalid UTF-8: {}", e)))
    }

    /// Delete a credential from the Keychain.
    pub fn delete(account: &str) -> Result<(), KeychainError> {
        security_framework::passwords::delete_generic_password(SERVICE_NAME, account)
            .map_err(|e| KeychainError::OperationFailed(e.to_string()))?;
        debug!(account, "deleted credential from keychain");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keychain_error_display_not_found() {
        let err = KeychainError::NotFound("my-api-key".to_string());
        assert_eq!(err.to_string(), "keychain item not found: my-api-key");
    }

    #[test]
    fn test_keychain_error_display_operation_failed() {
        let err = KeychainError::OperationFailed("access denied".to_string());
        assert_eq!(err.to_string(), "keychain error: access denied");
    }
}
