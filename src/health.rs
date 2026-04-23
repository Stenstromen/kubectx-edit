use std::time::{Duration, Instant};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use reqwest::blocking::Client;
use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue};

use crate::types::{Cluster, User, UserAuth};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);
const HEALTH_ENDPOINTS: &[&str] = &["/livez", "/healthz"];

#[derive(Debug, Clone)]
pub struct HealthCheckReport {
    pub cluster_name: String,
    pub server: String,
    pub outcome: HealthOutcome,
}

#[derive(Debug, Clone)]
pub enum HealthOutcome {
    Healthy {
        endpoint: String,
        status: u16,
        latency_ms: u128,
    },
    Unhealthy {
        endpoint: String,
        status: u16,
        body: String,
        latency_ms: u128,
    },
    Error(String),
}

impl HealthCheckReport {
    pub fn is_healthy(&self) -> bool {
        matches!(self.outcome, HealthOutcome::Healthy { .. })
    }

    pub fn summary(&self) -> String {
        match &self.outcome {
            HealthOutcome::Healthy {
                endpoint,
                status,
                latency_ms,
            } => format!(
                "OK  {} {}{} -> {} ({} ms)",
                self.cluster_name, self.server, endpoint, status, latency_ms
            ),
            HealthOutcome::Unhealthy {
                endpoint,
                status,
                body,
                latency_ms,
            } => {
                let snippet = body_snippet(body);
                format!(
                    "FAIL  {} {}{} -> {} ({}) ({} ms)",
                    self.cluster_name, self.server, endpoint, status, snippet, latency_ms
                )
            }
            HealthOutcome::Error(msg) => {
                format!("ERROR  {} ({}): {}", self.cluster_name, self.server, msg)
            }
        }
    }
}

/// Run a health check against the given cluster.
///
/// Issues a GET to the Kubernetes API server's health endpoint, using the
/// cluster's `certificate-authority-data` as an extra trust root and the
/// provided user's token or client certificate for authentication.
pub fn check_cluster(cluster: &Cluster, user: Option<&User>) -> HealthCheckReport {
    let server = cluster.cluster.server.trim_end_matches('/').to_string();
    let outcome = match build_client(cluster, user) {
        Ok(client) => run_check(&client, &server),
        Err(e) => HealthOutcome::Error(e),
    };
    HealthCheckReport {
        cluster_name: cluster.name.clone(),
        server,
        outcome,
    }
}

fn build_client(cluster: &Cluster, user: Option<&User>) -> Result<Client, String> {
    let mut builder = Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .connect_timeout(REQUEST_TIMEOUT);

    if let Some(ca_b64) = &cluster.cluster.certificate_authority_data {
        let ca_pem = STANDARD
            .decode(ca_b64.as_bytes())
            .map_err(|e| format!("invalid certificate-authority-data base64: {}", e))?;
        let cert = reqwest::Certificate::from_pem(&ca_pem)
            .map_err(|e| format!("invalid certificate-authority-data PEM: {}", e))?;
        builder = builder.add_root_certificate(cert);
    }

    let mut headers = HeaderMap::new();
    if let Some(user) = user {
        match &user.user.auth {
            UserAuth::Token { token } => {
                let mut val = HeaderValue::from_str(&format!("Bearer {}", token))
                    .map_err(|e| format!("invalid bearer token: {}", e))?;
                val.set_sensitive(true);
                headers.insert(AUTHORIZATION, val);
            }
            UserAuth::Certificate {
                client_certificate_data,
                client_key_data,
            } => {
                let cert_pem = STANDARD
                    .decode(client_certificate_data.as_bytes())
                    .map_err(|e| format!("invalid client-certificate-data base64: {}", e))?;
                let key_pem = STANDARD
                    .decode(client_key_data.as_bytes())
                    .map_err(|e| format!("invalid client-key-data base64: {}", e))?;

                let mut identity_pem =
                    Vec::with_capacity(cert_pem.len() + key_pem.len() + 1);
                identity_pem.extend_from_slice(&cert_pem);
                if !cert_pem.ends_with(b"\n") {
                    identity_pem.push(b'\n');
                }
                identity_pem.extend_from_slice(&key_pem);

                let identity = reqwest::Identity::from_pem(&identity_pem)
                    .map_err(|e| format!("invalid client identity: {}", e))?;
                builder = builder.identity(identity);
            }
        }
    }

    builder
        .default_headers(headers)
        .build()
        .map_err(|e| format!("failed to build http client: {}", e))
}

fn run_check(client: &Client, server: &str) -> HealthOutcome {
    let mut last_404: Option<(String, u128)> = None;

    for endpoint in HEALTH_ENDPOINTS {
        let url = format!("{}{}", server, endpoint);
        let start = Instant::now();
        match client.get(&url).send() {
            Ok(resp) => {
                let status = resp.status();
                let latency_ms = start.elapsed().as_millis();
                let code = status.as_u16();

                if code == 404 {
                    last_404 = Some((endpoint.to_string(), latency_ms));
                    continue;
                }

                return if status.is_success() {
                    HealthOutcome::Healthy {
                        endpoint: endpoint.to_string(),
                        status: code,
                        latency_ms,
                    }
                } else {
                    let body = resp.text().unwrap_or_default();
                    HealthOutcome::Unhealthy {
                        endpoint: endpoint.to_string(),
                        status: code,
                        body,
                        latency_ms,
                    }
                };
            }
            Err(e) => {
                return HealthOutcome::Error(format!("GET {} failed: {}", url, short_error(&e)));
            }
        }
    }

    match last_404 {
        Some((endpoint, latency_ms)) => HealthOutcome::Unhealthy {
            endpoint,
            status: 404,
            body: "no health endpoint found".to_string(),
            latency_ms,
        },
        None => HealthOutcome::Error("no health endpoint available".to_string()),
    }
}

fn body_snippet(body: &str) -> String {
    let first_line = body.lines().next().unwrap_or("").trim();
    const MAX: usize = 80;
    if first_line.len() <= MAX {
        first_line.to_string()
    } else {
        format!("{}…", &first_line[..MAX])
    }
}

fn short_error(err: &reqwest::Error) -> String {
    // reqwest errors can be verbose (they chain source errors); keep only the
    // top-level description so the single-line status bar stays readable.
    let full = err.to_string();
    full.lines().next().unwrap_or(&full).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ClusterDetails;

    #[test]
    fn healthy_summary_contains_cluster_endpoint_server_and_status() {
        let report = HealthCheckReport {
            cluster_name: "my-cluster".to_string(),
            server: "https://api.example.com:6443".to_string(),
            outcome: HealthOutcome::Healthy {
                endpoint: "/livez".to_string(),
                status: 200,
                latency_ms: 42,
            },
        };
        let summary = report.summary();
        assert!(summary.contains("my-cluster"));
        assert!(summary.contains("https://api.example.com:6443"));
        assert!(summary.contains("/livez"));
        assert!(summary.contains("200"));
        assert!(summary.contains("42"));
    }

    #[test]
    fn unhealthy_summary_contains_status_and_body_snippet() {
        let report = HealthCheckReport {
            cluster_name: "c".to_string(),
            server: "s".to_string(),
            outcome: HealthOutcome::Unhealthy {
                endpoint: "/livez".to_string(),
                status: 503,
                body: "something broke\nmore details".to_string(),
                latency_ms: 10,
            },
        };
        let summary = report.summary();
        assert!(summary.contains("503"));
        assert!(summary.contains("something broke"));
        assert!(!summary.contains("more details"));
    }

    #[test]
    fn error_summary_contains_message() {
        let report = HealthCheckReport {
            cluster_name: "c".to_string(),
            server: "s".to_string(),
            outcome: HealthOutcome::Error("boom".to_string()),
        };
        assert!(report.summary().contains("boom"));
    }

    #[test]
    fn build_client_rejects_invalid_ca_base64() {
        let cluster = Cluster {
            name: "bad-ca".to_string(),
            cluster: ClusterDetails {
                server: "https://example.com".to_string(),
                certificate_authority_data: Some("not-base64!!!".to_string()),
            },
        };
        let err = build_client(&cluster, None).unwrap_err();
        assert!(err.contains("certificate-authority-data"));
    }

    #[test]
    fn build_client_rejects_invalid_client_cert_base64() {
        let cluster = Cluster {
            name: "c".to_string(),
            cluster: ClusterDetails {
                server: "https://example.com".to_string(),
                certificate_authority_data: None,
            },
        };
        let user = User {
            name: "u".to_string(),
            user: crate::types::UserDetails {
                auth: UserAuth::Certificate {
                    client_certificate_data: "not-base64!!!".to_string(),
                    client_key_data: "also-not-base64!!!".to_string(),
                },
            },
        };
        let err = build_client(&cluster, Some(&user)).unwrap_err();
        assert!(err.contains("client-certificate-data"));
    }

    #[test]
    fn build_client_accepts_token_user() {
        let cluster = Cluster {
            name: "c".to_string(),
            cluster: ClusterDetails {
                server: "https://example.com".to_string(),
                certificate_authority_data: None,
            },
        };
        let user = User {
            name: "u".to_string(),
            user: crate::types::UserDetails {
                auth: UserAuth::Token {
                    token: "abc".to_string(),
                },
            },
        };
        assert!(build_client(&cluster, Some(&user)).is_ok());
    }
}
