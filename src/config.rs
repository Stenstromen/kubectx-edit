use crate::types::Config;
use serde_yaml::Value;
use std::{env, error::Error, fs, path::PathBuf};

pub fn load_config() -> Result<(Config, PathBuf), Box<dyn Error>> {
    let kubeconfig_path = get_kubeconfig_path();
    let yaml_content = fs::read_to_string(&kubeconfig_path)?;
    let config: Config = serde_yaml::from_str(&yaml_content)?;
    Ok((config, kubeconfig_path))
}

pub fn get_kubeconfig_path() -> PathBuf {
    env::var("KUBECONFIG")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let home = env::var("HOME").expect("HOME environment variable not set");
            PathBuf::from(home).join(".kube").join("config")
        })
}

pub fn save_config(config: &Config, path: &PathBuf) -> Result<(), Box<dyn Error>> {
    let existing_content = fs::read_to_string(path)?;
    let mut existing_yaml: Value = serde_yaml::from_str(&existing_content)?;

    if let Value::Mapping(ref mut mapping) = existing_yaml {
        mapping.insert(
            Value::String("clusters".to_string()),
            serde_yaml::to_value(&config.clusters)?,
        );
        mapping.insert(
            Value::String("users".to_string()),
            serde_yaml::to_value(&config.users)?,
        );
        mapping.insert(
            Value::String("contexts".to_string()),
            serde_yaml::to_value(&config.contexts)?,
        );

        // Update current-context
        if let Some(current_context) = &config.current_context {
            mapping.insert(
                Value::String("current-context".to_string()),
                Value::String(current_context.clone()),
            );
        } else {
            // Remove current-context if it's None
            mapping.remove(&Value::String("current-context".to_string()));
        }

        if !mapping.contains_key("apiVersion") {
            mapping.insert(
                Value::String("apiVersion".to_string()),
                Value::String("v1".to_string()),
            );
        }
        if !mapping.contains_key("kind") {
            mapping.insert(
                Value::String("kind".to_string()),
                Value::String("Config".to_string()),
            );
        }
    }

    let yaml_content = serde_yaml::to_string(&existing_yaml)?;
    fs::write(path, yaml_content)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_config() -> Config {
        Config {
            clusters: vec![Cluster {
                name: "test-cluster".to_string(),
                cluster: ClusterDetails {
                    server: "https://test-server:6443".to_string(),
                    certificate_authority_data: Some("test-ca-data".to_string()),
                },
            }],
            users: vec![User {
                name: "test-user".to_string(),
                user: UserDetails {
                    auth: UserAuth::Token {
                        token: "test-token".to_string(),
                    },
                },
            }],
            contexts: vec![Context {
                name: "test-context".to_string(),
                context: ContextDetails {
                    user: "test-user".to_string(),
                    cluster: "test-cluster".to_string(),
                },
            }],
            current_context: Some("test-context".to_string()),
            preferences: None,
        }
    }

    fn create_existing_yaml() -> String {
        r#"apiVersion: v1
kind: Config
clusters:
- cluster:
    server: https://old-server:6443
  name: old-cluster
users:
- name: old-user
  user:
    token: old-token
contexts:
- context:
    cluster: old-cluster
    user: old-user
  name: old-context
current-context: old-context
preferences: {}
"#
        .to_string()
    }

    #[test]
    fn test_save_config_successful_save() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config");

        // Create an existing config file
        fs::write(&config_path, create_existing_yaml()).unwrap();

        let test_config = create_test_config();
        let result = save_config(&test_config, &config_path);

        assert!(result.is_ok(), "save_config should succeed");

        // Verify the file was updated
        let saved_content = fs::read_to_string(&config_path).unwrap();
        let saved_yaml: Value = serde_yaml::from_str(&saved_content).unwrap();

        // Check that clusters were updated
        if let Some(clusters) = saved_yaml.get("clusters") {
            let clusters_vec: Vec<Cluster> = serde_yaml::from_value(clusters.clone()).unwrap();
            assert_eq!(clusters_vec.len(), 1);
            assert_eq!(clusters_vec[0].name, "test-cluster");
            assert_eq!(clusters_vec[0].cluster.server, "https://test-server:6443");
        } else {
            panic!("clusters field should exist");
        }

        // Check current-context was updated
        if let Some(current_context) = saved_yaml.get("current-context") {
            assert_eq!(current_context.as_str().unwrap(), "test-context");
        } else {
            panic!("current-context field should exist");
        }

        // Check that apiVersion and kind are preserved/added
        assert_eq!(
            saved_yaml.get("apiVersion").unwrap().as_str().unwrap(),
            "v1"
        );
        assert_eq!(saved_yaml.get("kind").unwrap().as_str().unwrap(), "Config");
    }

    #[test]
    fn test_save_config_with_no_current_context() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config");

        // Create an existing config file
        fs::write(&config_path, create_existing_yaml()).unwrap();

        let mut test_config = create_test_config();
        test_config.current_context = None;

        let result = save_config(&test_config, &config_path);
        assert!(
            result.is_ok(),
            "save_config should succeed with no current-context"
        );

        // Verify current-context was removed
        let saved_content = fs::read_to_string(&config_path).unwrap();
        let saved_yaml: Value = serde_yaml::from_str(&saved_content).unwrap();

        assert!(
            saved_yaml.get("current-context").is_none(),
            "current-context should be removed when None"
        );
    }

    #[test]
    fn test_save_config_preserves_existing_metadata() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config");

        // Create existing config without apiVersion and kind
        let minimal_yaml = r#"clusters:
- cluster:
    server: https://old-server:6443
  name: old-cluster
users:
- name: old-user
  user:
    token: old-token
contexts:
- context:
    cluster: old-cluster
    user: old-user
  name: old-context
"#;
        fs::write(&config_path, minimal_yaml).unwrap();

        let test_config = create_test_config();
        let result = save_config(&test_config, &config_path);

        assert!(result.is_ok(), "save_config should succeed");

        // Verify apiVersion and kind were added
        let saved_content = fs::read_to_string(&config_path).unwrap();
        let saved_yaml: Value = serde_yaml::from_str(&saved_content).unwrap();

        assert_eq!(
            saved_yaml.get("apiVersion").unwrap().as_str().unwrap(),
            "v1"
        );
        assert_eq!(saved_yaml.get("kind").unwrap().as_str().unwrap(), "Config");
    }

    #[test]
    fn test_save_config_handles_certificate_auth() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config");

        fs::write(&config_path, create_existing_yaml()).unwrap();

        let mut test_config = create_test_config();
        test_config.users[0].user.auth = UserAuth::Certificate {
            client_certificate_data: "test-cert-data".to_string(),
            client_key_data: "test-key-data".to_string(),
        };

        let result = save_config(&test_config, &config_path);
        assert!(
            result.is_ok(),
            "save_config should succeed with certificate auth"
        );

        // Verify certificate auth was saved correctly
        let saved_content = fs::read_to_string(&config_path).unwrap();
        let saved_yaml: Value = serde_yaml::from_str(&saved_content).unwrap();

        if let Some(users) = saved_yaml.get("users") {
            let users_vec: Vec<User> = serde_yaml::from_value(users.clone()).unwrap();
            match &users_vec[0].user.auth {
                UserAuth::Certificate {
                    client_certificate_data,
                    client_key_data,
                } => {
                    assert_eq!(client_certificate_data, "test-cert-data");
                    assert_eq!(client_key_data, "test-key-data");
                }
                _ => panic!("Expected certificate auth"),
            }
        }
    }

    #[test]
    fn test_save_config_file_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("nonexistent").join("config");

        let test_config = create_test_config();
        let result = save_config(&test_config, &config_path);

        assert!(
            result.is_err(),
            "save_config should fail when file doesn't exist"
        );
    }

    #[test]
    fn test_save_config_invalid_yaml() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config");

        // Create a file with invalid YAML
        fs::write(&config_path, "invalid: yaml: content: [").unwrap();

        let test_config = create_test_config();
        let result = save_config(&test_config, &config_path);

        assert!(result.is_err(), "save_config should fail with invalid YAML");
    }

    #[test]
    fn test_save_config_multiple_clusters_users_contexts() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config");

        fs::write(&config_path, create_existing_yaml()).unwrap();

        let mut test_config = create_test_config();

        // Add multiple items
        test_config.clusters.push(Cluster {
            name: "test-cluster-2".to_string(),
            cluster: ClusterDetails {
                server: "https://test-server-2:6443".to_string(),
                certificate_authority_data: None,
            },
        });

        test_config.users.push(User {
            name: "test-user-2".to_string(),
            user: UserDetails {
                auth: UserAuth::Certificate {
                    client_certificate_data: "cert-data-2".to_string(),
                    client_key_data: "key-data-2".to_string(),
                },
            },
        });

        test_config.contexts.push(Context {
            name: "test-context-2".to_string(),
            context: ContextDetails {
                user: "test-user-2".to_string(),
                cluster: "test-cluster-2".to_string(),
            },
        });

        let result = save_config(&test_config, &config_path);
        assert!(
            result.is_ok(),
            "save_config should succeed with multiple items"
        );

        // Verify all items were saved
        let saved_content = fs::read_to_string(&config_path).unwrap();
        let saved_yaml: Value = serde_yaml::from_str(&saved_content).unwrap();

        let clusters: Vec<Cluster> =
            serde_yaml::from_value(saved_yaml.get("clusters").unwrap().clone()).unwrap();
        let users: Vec<User> =
            serde_yaml::from_value(saved_yaml.get("users").unwrap().clone()).unwrap();
        let contexts: Vec<Context> =
            serde_yaml::from_value(saved_yaml.get("contexts").unwrap().clone()).unwrap();

        assert_eq!(clusters.len(), 2);
        assert_eq!(users.len(), 2);
        assert_eq!(contexts.len(), 2);

        assert_eq!(clusters[1].name, "test-cluster-2");
        assert_eq!(users[1].name, "test-user-2");
        assert_eq!(contexts[1].name, "test-context-2");
    }
}
