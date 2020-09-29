use std::collections::HashMap;

fn safe_get_env(env_var: &str) -> String {
    let val = std::env::var(env_var).unwrap_or_else(|_| panic!("Env var {} undefined", env_var));
    if val.is_empty() {
        panic!("Env var {} is empty", env_var);
    }
    val
}

#[derive(Debug, Clone)]
pub struct OIDCUser {
    pub email: String,
    pub refresh_token: String,
}

#[derive(Debug, Clone)]
pub struct OIDCConfig {
    pub client_id: String,
    pub client_secret: String,
    pub provider: String,
    pub users: HashMap<String, OIDCUser>,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub id_token: String,
    pub api_url: String,
    pub admin_url: String,
    pub oidc_config: OIDCConfig,
}

impl Config {
    pub fn new() -> Self {
        Self {
            id_token: safe_get_env("TANKER_ID_TOKEN"),
            api_url: safe_get_env("TANKER_TRUSTCHAIND_URL"),
            admin_url: safe_get_env("TANKER_ADMIND_URL"),
            oidc_config: OIDCConfig::new(),
        }
    }
}

impl OIDCConfig {
    pub fn new() -> Self {
        Self {
            client_id: safe_get_env("TANKER_OIDC_CLIENT_ID"),
            client_secret: safe_get_env("TANKER_OIDC_CLIENT_SECRET"),
            provider: safe_get_env("TANKER_OIDC_PROVIDER"),
            users: vec![
                (
                    "martine".into(),
                    OIDCUser {
                        email: safe_get_env("TANKER_OIDC_MARTINE_EMAIL"),
                        refresh_token: safe_get_env("TANKER_OIDC_MARTINE_REFRESH_TOKEN"),
                    },
                ),
                (
                    "kevin".into(),
                    OIDCUser {
                        email: safe_get_env("TANKER_OIDC_KEVIN_EMAIL"),
                        refresh_token: safe_get_env("TANKER_OIDC_KEVIN_REFRESH_TOKEN"),
                    },
                ),
            ]
            .into_iter()
            .collect(),
        }
    }
}
