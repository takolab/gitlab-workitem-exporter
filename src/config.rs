use std::env;
use std::error::Error;
use std::fmt;

/// Loads a local `.env` file if present, without overriding variables that
/// are already set in the process environment. Missing `.env` is not an
/// error: the application must run with plain environment variables alone.
pub fn load_dotenv() {
    let _ = dotenvy::dotenv();
}

pub struct MissingTokenError;

impl fmt::Display for MissingTokenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "GITLAB_TOKEN is not set. Copy .env.example to .env and set your GitLab token, \
             or export GITLAB_TOKEN in your shell."
        )
    }
}

// main() reports errors via their Debug output, so route it through
// Display to avoid printing the bare struct name instead of the message.
impl fmt::Debug for MissingTokenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl Error for MissingTokenError {}

pub struct Config {
    pub token: String,
    pub base_url: String,
}

impl fmt::Debug for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Config")
            .field("token", &"<redacted>")
            .field("base_url", &self.base_url)
            .finish()
    }
}

impl Config {
    pub fn from_env() -> Result<Config, Box<dyn Error>> {
        let token = env::var("GITLAB_TOKEN").unwrap_or_default();

        if token.trim().is_empty() {
            return Err(MissingTokenError.into());
        }

        let base_url =
            env::var("GITLAB_BASE_URL").unwrap_or_else(|_| "https://gitlab.com".to_string());

        Ok(Config { token, base_url })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // GITLAB_TOKEN / GITLAB_BASE_URL are process-wide state, so these tests
    // must not run concurrently with each other.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn clear_env() {
        unsafe {
            env::remove_var("GITLAB_TOKEN");
            env::remove_var("GITLAB_BASE_URL");
        }
    }

    #[test]
    fn errors_when_token_is_missing() {
        let _guard = ENV_LOCK.lock().unwrap();
        clear_env();

        let error = Config::from_env().expect_err("missing token should fail");

        assert_eq!(
            error.to_string(),
            "GITLAB_TOKEN is not set. Copy .env.example to .env and set your GitLab token, \
             or export GITLAB_TOKEN in your shell."
        );
    }

    #[test]
    fn errors_when_token_is_empty() {
        let _guard = ENV_LOCK.lock().unwrap();
        clear_env();
        unsafe {
            env::set_var("GITLAB_TOKEN", "");
        }

        let error = Config::from_env().expect_err("empty token should fail");

        assert_eq!(
            error.to_string(),
            "GITLAB_TOKEN is not set. Copy .env.example to .env and set your GitLab token, \
             or export GITLAB_TOKEN in your shell."
        );

        clear_env();
    }

    #[test]
    fn uses_default_base_url_when_unset() {
        let _guard = ENV_LOCK.lock().unwrap();
        clear_env();
        unsafe {
            env::set_var("GITLAB_TOKEN", "test-token");
        }

        let config = Config::from_env().expect("token is set");

        assert_eq!(config.token, "test-token");
        assert_eq!(config.base_url, "https://gitlab.com");

        clear_env();
    }

    #[test]
    fn uses_custom_base_url_when_set() {
        let _guard = ENV_LOCK.lock().unwrap();
        clear_env();
        unsafe {
            env::set_var("GITLAB_TOKEN", "test-token");
            env::set_var("GITLAB_BASE_URL", "https://gitlab.example.com");
        }

        let config = Config::from_env().expect("token is set");

        assert_eq!(config.base_url, "https://gitlab.example.com");

        clear_env();
    }
}
