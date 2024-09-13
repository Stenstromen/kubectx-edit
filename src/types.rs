use serde::{Deserialize, Serialize};
use serde_yaml::Value;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub clusters: Vec<Cluster>,
    pub users: Vec<User>,
    pub contexts: Vec<Context>,
    #[serde(rename = "current-context", skip_serializing_if = "Option::is_none")]
    pub current_context: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preferences: Option<Value>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Cluster {
    pub name: String,
    pub cluster: ClusterDetails,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ClusterDetails {
    pub server: String,
    #[serde(rename = "certificate-authority-data", skip_serializing_if = "Option::is_none")]
    pub certificate_authority_data: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct User {
    pub name: String,
    pub user: UserDetails,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UserDetails {
    #[serde(flatten)]
    pub auth: UserAuth,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum UserAuth {
    Token {
        token: String,
    },
    Certificate {
        #[serde(rename = "client-certificate-data")]
        client_certificate_data: String,
        #[serde(rename = "client-key-data")]
        client_key_data: String,
    },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Context {
    pub name: String,
    pub context: ContextDetails,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ContextDetails {
    pub user: String,
    pub cluster: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TempConfig {
    pub cluster: Cluster,
    pub context: Option<Context>,
    pub user: Option<User>,
}