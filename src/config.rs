use std::collections::HashSet;
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

/// A plain, message-only error used for configuration and CLI argument
/// resolution failures. Its Debug output mirrors Display so that errors
/// bubbling up through `main()` print a clean, single-line message.
pub struct ConfigError(String);

impl ConfigError {
    fn new(message: impl Into<String>) -> Self {
        ConfigError(message.into())
    }
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl fmt::Debug for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl Error for ConfigError {}

/// The two ways this CLI can export Work Items: a single Work Item with all
/// of its comments, or multiple Work Items from the same GitLab project,
/// each with a limited number of recent non-system comments.
#[derive(Debug, PartialEq, Eq)]
pub enum ExportMode {
    Single(u64),
    Multiple(Vec<u64>),
}

/// Parses a comma-separated list of Work Item IIDs (from `--iids` or
/// `GITLAB_WORK_ITEM_IIDS`): trims whitespace around each entry, ignores
/// empty entries, rejects non-positive-integer entries with a message that
/// names the offending value, and removes duplicates while keeping the
/// first-seen order.
pub fn parse_iid_list(raw: &str, source_label: &str) -> Result<Vec<u64>, ConfigError> {
    let mut iids = Vec::new();
    let mut seen = HashSet::new();

    for token in raw.split(',') {
        let trimmed = token.trim();

        if trimmed.is_empty() {
            continue;
        }

        let iid: u64 = trimmed.parse().ok().filter(|iid| *iid > 0).ok_or_else(|| {
            ConfigError::new(format!(
                "Invalid Work Item IID '{trimmed}' in {source_label}: must be a positive integer"
            ))
        })?;

        if seen.insert(iid) {
            iids.push(iid);
        }
    }

    if iids.is_empty() {
        return Err(ConfigError::new(format!(
            "No valid Work Item IIDs found in {source_label}"
        )));
    }

    Ok(iids)
}

/// Resolves which export mode to use from CLI arguments, falling back to
/// `GITLAB_WORK_ITEM_IIDS` only when neither `--iid` nor `--iids` is given.
/// `GITLAB_WORK_ITEM_IIDS` is read here, not eagerly at startup, so an
/// invalid value never affects an explicit `--iid` single export.
pub fn resolve_export_mode(
    cli_iid: Option<u64>,
    cli_iids: Option<&str>,
) -> Result<ExportMode, ConfigError> {
    match (cli_iid, cli_iids) {
        (Some(_), Some(_)) => Err(ConfigError::new(
            "--iid and --iids are mutually exclusive; specify only one.",
        )),
        (Some(0), None) => Err(ConfigError::new(
            "Invalid Work Item IID '0' in --iid: must be a positive integer",
        )),
        (Some(iid), None) => Ok(ExportMode::Single(iid)),
        (None, Some(raw)) => parse_iid_list(raw, "--iids").map(ExportMode::Multiple),
        (None, None) => {
            let raw = env::var("GITLAB_WORK_ITEM_IIDS").map_err(|_| {
                ConfigError::new(
                    "No Work Item IID specified. Use --iid <IID> for a single export, \
                     --iids <IID,IID,...> for multiple, or set GITLAB_WORK_ITEM_IIDS in \
                     the environment.",
                )
            })?;

            parse_iid_list(&raw, "GITLAB_WORK_ITEM_IIDS").map(ExportMode::Multiple)
        }
    }
}

/// Reads `GITLAB_RECENT_COMMENTS_LIMIT` (default 10). Only called once a
/// multi Work Item export has been selected, so an invalid value never
/// affects a single Work Item export.
pub fn recent_comments_limit_from_env() -> Result<usize, ConfigError> {
    match env::var("GITLAB_RECENT_COMMENTS_LIMIT") {
        Ok(raw) => {
            let trimmed = raw.trim();

            trimmed
                .parse::<usize>()
                .ok()
                .filter(|limit| *limit > 0)
                .ok_or_else(|| {
                    ConfigError::new(format!(
                        "GITLAB_RECENT_COMMENTS_LIMIT must be a positive integer, got '{trimmed}'"
                    ))
                })
        }
        Err(_) => Ok(10),
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
            env::remove_var("GITLAB_WORK_ITEM_IIDS");
            env::remove_var("GITLAB_RECENT_COMMENTS_LIMIT");
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

    #[test]
    fn parse_iid_list_trims_whitespace_and_skips_empty_entries() {
        let iids = parse_iid_list(" 23, 24 ,25, ,", "--iids").expect("should parse");

        assert_eq!(iids, vec![23, 24, 25]);
    }

    #[test]
    fn parse_iid_list_deduplicates_preserving_first_occurrence_order() {
        let iids = parse_iid_list("23,24,23,25,24", "--iids").expect("should parse");

        assert_eq!(iids, vec![23, 24, 25]);
    }

    #[test]
    fn parse_iid_list_rejects_non_numeric_entry() {
        let error = parse_iid_list("23,abc,25", "--iids").expect_err("should fail");

        assert_eq!(
            error.to_string(),
            "Invalid Work Item IID 'abc' in --iids: must be a positive integer"
        );
    }

    #[test]
    fn parse_iid_list_rejects_zero() {
        let error = parse_iid_list("23,0,25", "--iids").expect_err("should fail");

        assert_eq!(
            error.to_string(),
            "Invalid Work Item IID '0' in --iids: must be a positive integer"
        );
    }

    #[test]
    fn parse_iid_list_errors_when_no_valid_iids_remain() {
        let error = parse_iid_list(" , ,", "GITLAB_WORK_ITEM_IIDS").expect_err("should fail");

        assert_eq!(
            error.to_string(),
            "No valid Work Item IIDs found in GITLAB_WORK_ITEM_IIDS"
        );
    }

    #[test]
    fn resolve_export_mode_uses_single_when_only_iid_given() {
        let mode = resolve_export_mode(Some(30), None).expect("should resolve");

        assert_eq!(mode, ExportMode::Single(30));
    }

    #[test]
    fn resolve_export_mode_uses_multiple_when_only_iids_given() {
        let mode = resolve_export_mode(None, Some("23,24,25")).expect("should resolve");

        assert_eq!(mode, ExportMode::Multiple(vec![23, 24, 25]));
    }

    #[test]
    fn resolve_export_mode_errors_when_both_iid_and_iids_given() {
        let error = resolve_export_mode(Some(30), Some("23,24"))
            .expect_err("should reject mutually exclusive flags");

        assert_eq!(
            error.to_string(),
            "--iid and --iids are mutually exclusive; specify only one."
        );
    }

    #[test]
    fn resolve_export_mode_rejects_zero_iid() {
        let error = resolve_export_mode(Some(0), None).expect_err("zero IID should fail");

        assert_eq!(
            error.to_string(),
            "Invalid Work Item IID '0' in --iid: must be a positive integer"
        );
    }

    #[test]
    fn resolve_export_mode_prefers_cli_iid_over_env_work_item_iids() {
        let _guard = ENV_LOCK.lock().unwrap();
        clear_env();
        unsafe {
            env::set_var("GITLAB_WORK_ITEM_IIDS", "not valid at all");
        }

        let mode = resolve_export_mode(Some(30), None)
            .expect("explicit --iid should ignore invalid env IIDs");

        assert_eq!(mode, ExportMode::Single(30));

        clear_env();
    }

    #[test]
    fn resolve_export_mode_prefers_cli_iids_over_env_work_item_iids() {
        let _guard = ENV_LOCK.lock().unwrap();
        clear_env();
        unsafe {
            env::set_var("GITLAB_WORK_ITEM_IIDS", "101,102");
        }

        let mode =
            resolve_export_mode(None, Some("23,24")).expect("explicit --iids should take over");

        assert_eq!(mode, ExportMode::Multiple(vec![23, 24]));

        clear_env();
    }

    #[test]
    fn resolve_export_mode_falls_back_to_env_work_item_iids() {
        let _guard = ENV_LOCK.lock().unwrap();
        clear_env();
        unsafe {
            env::set_var("GITLAB_WORK_ITEM_IIDS", "101,102,103");
        }

        let mode = resolve_export_mode(None, None).expect("should fall back to env");

        assert_eq!(mode, ExportMode::Multiple(vec![101, 102, 103]));

        clear_env();
    }

    #[test]
    fn resolve_export_mode_errors_when_nothing_is_specified() {
        let _guard = ENV_LOCK.lock().unwrap();
        clear_env();

        let error = resolve_export_mode(None, None).expect_err("should fail without any IID");

        assert_eq!(
            error.to_string(),
            "No Work Item IID specified. Use --iid <IID> for a single export, --iids <IID,IID,...> for multiple, or set GITLAB_WORK_ITEM_IIDS in the environment."
        );
    }

    #[test]
    fn recent_comments_limit_defaults_to_ten() {
        let _guard = ENV_LOCK.lock().unwrap();
        clear_env();

        let limit = recent_comments_limit_from_env().expect("should default");

        assert_eq!(limit, 10);
    }

    #[test]
    fn recent_comments_limit_reads_custom_value() {
        let _guard = ENV_LOCK.lock().unwrap();
        clear_env();
        unsafe {
            env::set_var("GITLAB_RECENT_COMMENTS_LIMIT", "5");
        }

        let limit = recent_comments_limit_from_env().expect("should parse");

        assert_eq!(limit, 5);

        clear_env();
    }

    #[test]
    fn recent_comments_limit_rejects_zero() {
        let _guard = ENV_LOCK.lock().unwrap();
        clear_env();
        unsafe {
            env::set_var("GITLAB_RECENT_COMMENTS_LIMIT", "0");
        }

        let error = recent_comments_limit_from_env().expect_err("zero should fail");

        assert_eq!(
            error.to_string(),
            "GITLAB_RECENT_COMMENTS_LIMIT must be a positive integer, got '0'"
        );

        clear_env();
    }

    #[test]
    fn recent_comments_limit_rejects_non_numeric_value() {
        let _guard = ENV_LOCK.lock().unwrap();
        clear_env();
        unsafe {
            env::set_var("GITLAB_RECENT_COMMENTS_LIMIT", "abc");
        }

        let error = recent_comments_limit_from_env().expect_err("non-numeric should fail");

        assert_eq!(
            error.to_string(),
            "GITLAB_RECENT_COMMENTS_LIMIT must be a positive integer, got 'abc'"
        );

        clear_env();
    }
}
