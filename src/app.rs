use crate::config;
use crate::health;
use crate::types::{Cluster, Config, TempConfig};
use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::widgets::ListState;
use std::{env, io, path::PathBuf, process::Command};
use tempfile::NamedTempFile;

pub struct App {
    pub config: Config,
    pub cluster_list_state: ListState,
    pub selected_cluster: Option<Cluster>,
    pub needs_redraw: bool,
    pub kubeconfig_path: PathBuf,
    pub status_message: Option<String>,
}

impl App {
    pub fn new(config: Config, kubeconfig_path: PathBuf) -> Self {
        let mut app = Self {
            config,
            cluster_list_state: ListState::default(),
            selected_cluster: None,
            needs_redraw: false,
            kubeconfig_path,
            status_message: None,
        };

        // Select first item if there are any clusters
        if !app.config.clusters.is_empty() {
            app.cluster_list_state.select(Some(0));
            app.selected_cluster = Some(app.config.clusters[0].clone());
        }

        app
    }

    pub fn next(&mut self) {
        let i = match self.cluster_list_state.selected() {
            Some(i) => (i + 1) % self.config.clusters.len(),
            None => 0,
        };
        self.cluster_list_state.select(Some(i));
        self.selected_cluster = Some(self.config.clusters[i].clone());
    }

    pub fn previous(&mut self) {
        let i = match self.cluster_list_state.selected() {
            Some(i) => (i + self.config.clusters.len() - 1) % self.config.clusters.len(),
            None => 0,
        };
        self.cluster_list_state.select(Some(i));
        self.selected_cluster = Some(self.config.clusters[i].clone());
    }

    pub fn select(&mut self) {
        if let Some(i) = self.cluster_list_state.selected() {
            self.selected_cluster = Some(self.config.clusters[i].clone());
        }
    }

    pub fn delete_selected(&mut self) {
        if let Some(selected) = self.cluster_list_state.selected() {
            let cluster_name = self.config.clusters[selected].name.clone();

            // Remove the cluster
            self.config.clusters.remove(selected);

            // Find and store user information before removing context
            let user_to_remove = self
                .config
                .contexts
                .iter()
                .find(|c| c.context.cluster == cluster_name)
                .map(|c| c.context.user.clone());

            // Remove associated context
            if let Some(context_index) = self
                .config
                .contexts
                .iter()
                .position(|c| c.context.cluster == cluster_name)
            {
                let context_name = self.config.contexts[context_index].name.clone();
                self.config.contexts.remove(context_index);

                // If the deleted context was the current context, update it
                if let Some(current) = &self.config.current_context {
                    if current == &context_name {
                        // Set current context to the first available context, or None if none exist
                        self.config.current_context =
                            self.config.contexts.first().map(|c| c.name.clone());
                    }
                }
            }

            // Remove associated user if it's not used by any other context
            if let Some(user_name) = user_to_remove {
                if !self
                    .config
                    .contexts
                    .iter()
                    .any(|c| c.context.user == user_name)
                {
                    if let Some(user_index) =
                        self.config.users.iter().position(|u| u.name == user_name)
                    {
                        self.config.users.remove(user_index);
                    }
                }
            }

            self.cluster_list_state
                .select(Some(selected.saturating_sub(1)));
            self.save_config().expect("Failed to save config");
            self.needs_redraw = true;
        }
    }

    pub fn edit_selected(&mut self) -> io::Result<()> {
        if let Some(cluster) = &self.selected_cluster {
            let editor = env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());
            let temp_file = NamedTempFile::new()?;

            let context = self
                .config
                .contexts
                .iter()
                .find(|c| c.context.cluster == cluster.name)
                .cloned();
            let user = context
                .as_ref()
                .and_then(|c| {
                    self.config
                        .users
                        .iter()
                        .find(|u| &u.name == &c.context.user)
                })
                .cloned();

            let temp_config = TempConfig {
                cluster: cluster.clone(),
                context,
                user,
            };

            serde_yaml::to_writer(&temp_file, &temp_config)
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

            crossterm::terminal::disable_raw_mode()?;
            execute!(io::stdout(), LeaveAlternateScreen)?;

            let status = Command::new(&editor).arg(temp_file.path()).status()?;

            execute!(io::stdout(), EnterAlternateScreen)?;
            crossterm::terminal::enable_raw_mode()?;

            let mut stdout = io::stdout();
            execute!(
                stdout,
                crossterm::terminal::Clear(crossterm::terminal::ClearType::All),
                crossterm::cursor::MoveTo(0, 0)
            )?;

            if status.success() {
                let edited_content = std::fs::read_to_string(temp_file.path())?;
                match serde_yaml::from_str::<TempConfig>(&edited_content) {
                    Ok(edited_config) => {
                        // Store original names for finding existing entries
                        let original_context = self
                            .config
                            .contexts
                            .iter()
                            .find(|c| c.context.cluster == cluster.name)
                            .cloned();
                        let original_user = original_context
                            .as_ref()
                            .and_then(|c| {
                                self.config
                                    .users
                                    .iter()
                                    .find(|u| &u.name == &c.context.user)
                            })
                            .cloned();

                        // Update cluster
                        if let Some(index) = self
                            .config
                            .clusters
                            .iter()
                            .position(|c| c.name == cluster.name)
                        {
                            self.config.clusters[index] = edited_config.cluster;
                            self.selected_cluster = Some(self.config.clusters[index].clone());
                        }

                        // Update context - find by original context name, not cluster name
                        if let Some(context) = edited_config.context {
                            if let Some(orig_context) = original_context {
                                if let Some(index) = self
                                    .config
                                    .contexts
                                    .iter()
                                    .position(|c| c.name == orig_context.name)
                                {
                                    self.config.contexts[index] = context;
                                } else {
                                    self.config.contexts.push(context);
                                }
                            } else {
                                self.config.contexts.push(context);
                            }
                        }

                        // Update user - find by original user name
                        if let Some(user) = edited_config.user {
                            if let Some(orig_user) = original_user {
                                if let Some(index) = self
                                    .config
                                    .users
                                    .iter()
                                    .position(|u| u.name == orig_user.name)
                                {
                                    self.config.users[index] = user;
                                } else {
                                    self.config.users.push(user);
                                }
                            } else {
                                self.config.users.push(user);
                            }
                        }

                        let updated_config = Config {
                            clusters: self.config.clusters.clone(),
                            users: self.config.users.clone(),
                            contexts: self.config.contexts.clone(),
                            current_context: self.config.current_context.clone(),
                            preferences: self.config.preferences.clone(),
                        };
                        let yaml_content = serde_yaml::to_string(&updated_config)
                            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
                        std::fs::write(&self.kubeconfig_path, yaml_content)?;
                    }
                    Err(e) => eprintln!("Failed to parse edited config: {}", e),
                }
            } else {
                eprintln!("Editor exited with non-zero status");
            }
        }
        self.save_config().expect("Failed to save config");
        self.needs_redraw = true;
        Ok(())
    }

    pub fn add_new_kubeconfig(&mut self) -> io::Result<()> {
        let editor = env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());
        let temp_file = NamedTempFile::new()?;

        // Create a template for the user to fill out
        let template = r#"# Add your new kubeconfig context below
# Replace the placeholders with your actual values

cluster:
  name: "new-cluster-name"
  cluster:
    server: "https://your-cluster-server:6443"
    certificate-authority-data: "YOUR_CA_DATA_HERE"

context:
  name: "new-context-name"
  context:
    cluster: "new-cluster-name"
    user: "new-user-name"

user:
  name: "new-user-name"
  user:
    token: "YOUR_TOKEN_HERE"
    # OR use certificate-based auth instead:
    # client-certificate-data: "YOUR_CLIENT_CERT_DATA"
    # client-key-data: "YOUR_CLIENT_KEY_DATA"
"#;

        std::fs::write(temp_file.path(), template)?;

        crossterm::terminal::disable_raw_mode()?;
        execute!(io::stdout(), LeaveAlternateScreen)?;

        let status = Command::new(&editor).arg(temp_file.path()).status()?;

        execute!(io::stdout(), EnterAlternateScreen)?;
        crossterm::terminal::enable_raw_mode()?;

        let mut stdout = io::stdout();
        execute!(
            stdout,
            crossterm::terminal::Clear(crossterm::terminal::ClearType::All),
            crossterm::cursor::MoveTo(0, 0)
        )?;

        if status.success() {
            let edited_content = std::fs::read_to_string(temp_file.path())?;
            match serde_yaml::from_str::<TempConfig>(&edited_content) {
                Ok(new_config) => {
                    // Check for duplicate names
                    if self
                        .config
                        .clusters
                        .iter()
                        .any(|c| c.name == new_config.cluster.name)
                    {
                        eprintln!(
                            "Error: Cluster '{}' already exists",
                            new_config.cluster.name
                        );
                        return Ok(());
                    }

                    if let Some(ref context) = new_config.context {
                        if self.config.contexts.iter().any(|c| c.name == context.name) {
                            eprintln!("Error: Context '{}' already exists", context.name);
                            return Ok(());
                        }
                    }

                    if let Some(ref user) = new_config.user {
                        if self.config.users.iter().any(|u| u.name == user.name) {
                            eprintln!("Error: User '{}' already exists", user.name);
                            return Ok(());
                        }
                    }

                    // Add the new entries
                    self.config.clusters.push(new_config.cluster);

                    if let Some(context) = new_config.context {
                        self.config.contexts.push(context);
                    }

                    if let Some(user) = new_config.user {
                        self.config.users.push(user);
                    }

                    let updated_config = Config {
                        clusters: self.config.clusters.clone(),
                        users: self.config.users.clone(),
                        contexts: self.config.contexts.clone(),
                        current_context: self.config.current_context.clone(),
                        preferences: self.config.preferences.clone(),
                    };
                    let yaml_content = serde_yaml::to_string(&updated_config)
                        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
                    std::fs::write(&self.kubeconfig_path, yaml_content)?;

                    println!("Successfully added new kubeconfig entry!");
                }
                Err(e) => eprintln!("Failed to parse new kubeconfig: {}", e),
            }
        } else {
            eprintln!("Editor exited with non-zero status");
        }

        self.save_config().expect("Failed to save config");
        self.needs_redraw = true;
        Ok(())
    }

    pub fn rotate_credentials(&mut self) -> io::Result<()> {
        if let Some(cluster) = &self.selected_cluster {
            let editor = env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());
            let temp_file = NamedTempFile::new()?;

            // Find the context and user for this cluster
            let context = self
                .config
                .contexts
                .iter()
                .find(|c| c.context.cluster == cluster.name)
                .cloned();

            let user = context
                .as_ref()
                .and_then(|c| {
                    self.config
                        .users
                        .iter()
                        .find(|u| &u.name == &c.context.user)
                })
                .cloned();

            if let Some(user) = &user {
                // Create a template with current credentials to rotate
                let template = format!(
                    r#"# Rotate credentials for cluster: {}
# Update the authentication details below:

user:
  name: "{}"
  user:
{}
"#,
                    cluster.name,
                    user.name,
                    match &user.user.auth {
                        crate::types::UserAuth::Token { token } => {
                            format!("    token: \"{}\"", token)
                        }
                        crate::types::UserAuth::Certificate {
                            client_certificate_data,
                            client_key_data,
                        } => {
                            format!(
                                "    client-certificate-data: \"{}\"\n    client-key-data: \"{}\"",
                                client_certificate_data, client_key_data
                            )
                        }
                    }
                );

                std::fs::write(temp_file.path(), template)?;

                crossterm::terminal::disable_raw_mode()?;
                execute!(io::stdout(), LeaveAlternateScreen)?;

                let status = Command::new(&editor).arg(temp_file.path()).status()?;

                execute!(io::stdout(), EnterAlternateScreen)?;
                crossterm::terminal::enable_raw_mode()?;

                let mut stdout = io::stdout();
                execute!(
                    stdout,
                    crossterm::terminal::Clear(crossterm::terminal::ClearType::All),
                    crossterm::cursor::MoveTo(0, 0)
                )?;

                if status.success() {
                    let edited_content = std::fs::read_to_string(temp_file.path())?;

                    // Parse as a simplified structure containing just the user
                    #[derive(serde::Deserialize)]
                    struct RotateConfig {
                        user: crate::types::User,
                    }

                    match serde_yaml::from_str::<RotateConfig>(&edited_content) {
                        Ok(rotate_config) => {
                            // Update the user's credentials
                            if let Some(index) = self
                                .config
                                .users
                                .iter()
                                .position(|u| u.name == rotate_config.user.name)
                            {
                                self.config.users[index] = rotate_config.user;

                                // Save the updated config
                                let updated_config = Config {
                                    clusters: self.config.clusters.clone(),
                                    users: self.config.users.clone(),
                                    contexts: self.config.contexts.clone(),
                                    current_context: self.config.current_context.clone(),
                                    preferences: self.config.preferences.clone(),
                                };
                                let yaml_content = serde_yaml::to_string(&updated_config)
                                    .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
                                std::fs::write(&self.kubeconfig_path, yaml_content)?;

                                println!("Successfully rotated credentials for '{}'!", user.name);
                            } else {
                                eprintln!("Error: User '{}' not found", rotate_config.user.name);
                            }
                        }
                        Err(e) => eprintln!("Failed to parse edited credentials: {}", e),
                    }
                } else {
                    eprintln!("Editor exited with non-zero status");
                }
            } else {
                eprintln!("No user found for cluster '{}'", cluster.name);
            }
        }

        self.save_config().expect("Failed to save config");
        self.needs_redraw = true;
        Ok(())
    }

    pub fn health_check(&mut self) {
        let message = match &self.selected_cluster {
            Some(cluster) => {
                let user = self
                    .config
                    .contexts
                    .iter()
                    .find(|c| c.context.cluster == cluster.name)
                    .and_then(|ctx| {
                        self.config
                            .users
                            .iter()
                            .find(|u| u.name == ctx.context.user)
                    });
                health::check_cluster(cluster, user).summary()
            }
            None => "No cluster selected".to_string(),
        };
        self.status_message = Some(message);
        self.needs_redraw = true;
    }

    pub fn save_config(&self) -> Result<(), Box<dyn std::error::Error>> {
        config::save_config(&self.config, &self.kubeconfig_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{
        Cluster, ClusterDetails, Context, ContextDetails, User, UserAuth, UserDetails,
    };
    use std::fs;
    use tempfile::TempDir;

    fn create_test_config() -> Config {
        Config {
            clusters: vec![
                Cluster {
                    name: "test-cluster".to_string(),
                    cluster: ClusterDetails {
                        server: "https://test-server:6443".to_string(),
                        certificate_authority_data: Some("test-ca-data".to_string()),
                    },
                },
                Cluster {
                    name: "test-cluster-2".to_string(),
                    cluster: ClusterDetails {
                        server: "https://test-server-2:6443".to_string(),
                        certificate_authority_data: Some("test-ca-data-2".to_string()),
                    },
                },
            ],
            contexts: vec![
                Context {
                    name: "test-context".to_string(),
                    context: ContextDetails {
                        cluster: "test-cluster".to_string(),
                        user: "test-user".to_string(),
                    },
                },
                Context {
                    name: "test-context-2".to_string(),
                    context: ContextDetails {
                        cluster: "test-cluster-2".to_string(),
                        user: "test-user-cert".to_string(),
                    },
                },
            ],
            users: vec![
                User {
                    name: "test-user".to_string(),
                    user: UserDetails {
                        auth: UserAuth::Token {
                            token: "old-token-value".to_string(),
                        },
                    },
                },
                User {
                    name: "test-user-cert".to_string(),
                    user: UserDetails {
                        auth: UserAuth::Certificate {
                            client_certificate_data: "old-cert-data".to_string(),
                            client_key_data: "old-key-data".to_string(),
                        },
                    },
                },
            ],
            current_context: Some("test-context".to_string()),
            preferences: None,
        }
    }

    #[test]
    fn test_app_creation() {
        let config = create_test_config();
        let temp_dir = TempDir::new().unwrap();
        let kubeconfig_path = temp_dir.path().join("config");

        let app = App::new(config.clone(), kubeconfig_path);

        assert_eq!(app.config.clusters.len(), 2);
        assert_eq!(app.config.users.len(), 2);
        assert_eq!(app.config.contexts.len(), 2);
        assert!(app.cluster_list_state.selected().is_some());
        assert_eq!(app.cluster_list_state.selected(), Some(0));
    }

    #[test]
    fn test_navigation() {
        let config = create_test_config();
        let temp_dir = TempDir::new().unwrap();
        let kubeconfig_path = temp_dir.path().join("config");
        let mut app = App::new(config, kubeconfig_path);

        // Initially selected cluster should be the first one
        assert_eq!(app.selected_cluster.as_ref().unwrap().name, "test-cluster");

        // Test next
        app.next();
        assert_eq!(app.cluster_list_state.selected(), Some(1));
        assert_eq!(app.selected_cluster.as_ref().unwrap().name, "test-cluster-2");

        app.next();
        assert_eq!(app.cluster_list_state.selected(), Some(0)); // Wraps around
        assert_eq!(app.selected_cluster.as_ref().unwrap().name, "test-cluster");

        // Test previous
        app.previous();
        assert_eq!(app.cluster_list_state.selected(), Some(1));
        assert_eq!(app.selected_cluster.as_ref().unwrap().name, "test-cluster-2");

        app.previous();
        assert_eq!(app.cluster_list_state.selected(), Some(0));
        assert_eq!(app.selected_cluster.as_ref().unwrap().name, "test-cluster");
    }

    #[test]
    fn test_user_credentials_update_token() {
        let mut config = create_test_config();

        // Find and update the token user
        if let Some(user) = config.users.iter_mut().find(|u| u.name == "test-user") {
            user.user.auth = UserAuth::Token {
                token: "new-token-value".to_string(),
            };
        }

        // Verify the update
        let updated_user = config.users.iter().find(|u| u.name == "test-user").unwrap();
        match &updated_user.user.auth {
            UserAuth::Token { token } => assert_eq!(token, "new-token-value"),
            _ => panic!("Expected token auth"),
        }
    }

    #[test]
    fn test_user_credentials_update_certificate() {
        let mut config = create_test_config();

        // Find and update the certificate user
        if let Some(user) = config.users.iter_mut().find(|u| u.name == "test-user-cert") {
            user.user.auth = UserAuth::Certificate {
                client_certificate_data: "new-cert-data".to_string(),
                client_key_data: "new-key-data".to_string(),
            };
        }

        // Verify the update
        let updated_user = config
            .users
            .iter()
            .find(|u| u.name == "test-user-cert")
            .unwrap();
        match &updated_user.user.auth {
            UserAuth::Certificate {
                client_certificate_data,
                client_key_data,
            } => {
                assert_eq!(client_certificate_data, "new-cert-data");
                assert_eq!(client_key_data, "new-key-data");
            }
            _ => panic!("Expected certificate auth"),
        }
    }

    #[test]
    fn test_find_user_for_cluster() {
        let config = create_test_config();

        // Find context for cluster
        let context = config
            .contexts
            .iter()
            .find(|c| c.context.cluster == "test-cluster")
            .unwrap();

        assert_eq!(context.name, "test-context");
        assert_eq!(context.context.user, "test-user");

        // Find user from context
        let user = config
            .users
            .iter()
            .find(|u| u.name == context.context.user)
            .unwrap();

        assert_eq!(user.name, "test-user");
        match &user.user.auth {
            UserAuth::Token { token } => assert_eq!(token, "old-token-value"),
            _ => panic!("Expected token auth"),
        }
    }

    #[test]
    fn test_config_serialization_with_token() {
        let user = User {
            name: "test-user".to_string(),
            user: UserDetails {
                auth: UserAuth::Token {
                    token: "test-token".to_string(),
                },
            },
        };

        let yaml = serde_yaml::to_string(&user).unwrap();
        assert!(yaml.contains("test-user"));
        assert!(yaml.contains("test-token"));

        // Test deserialization
        let deserialized: User = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(deserialized.name, "test-user");
        match deserialized.user.auth {
            UserAuth::Token { token } => assert_eq!(token, "test-token"),
            _ => panic!("Expected token auth"),
        }
    }

    #[test]
    fn test_config_serialization_with_certificate() {
        let user = User {
            name: "test-user".to_string(),
            user: UserDetails {
                auth: UserAuth::Certificate {
                    client_certificate_data: "cert-data".to_string(),
                    client_key_data: "key-data".to_string(),
                },
            },
        };

        let yaml = serde_yaml::to_string(&user).unwrap();
        assert!(yaml.contains("test-user"));
        assert!(yaml.contains("cert-data"));
        assert!(yaml.contains("key-data"));
        assert!(yaml.contains("client-certificate-data"));
        assert!(yaml.contains("client-key-data"));

        // Test deserialization
        let deserialized: User = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(deserialized.name, "test-user");
        match deserialized.user.auth {
            UserAuth::Certificate {
                client_certificate_data,
                client_key_data,
            } => {
                assert_eq!(client_certificate_data, "cert-data");
                assert_eq!(client_key_data, "key-data");
            }
            _ => panic!("Expected certificate auth"),
        }
    }

    #[test]
    fn test_rotate_config_structure() {
        // Test the RotateConfig structure that's used in rotate_credentials
        #[derive(serde::Deserialize, serde::Serialize)]
        struct RotateConfig {
            user: User,
        }

        let rotate_config = RotateConfig {
            user: User {
                name: "test-user".to_string(),
                user: UserDetails {
                    auth: UserAuth::Token {
                        token: "rotated-token".to_string(),
                    },
                },
            },
        };

        let yaml = serde_yaml::to_string(&rotate_config).unwrap();
        let deserialized: RotateConfig = serde_yaml::from_str(&yaml).unwrap();

        assert_eq!(deserialized.user.name, "test-user");
        match deserialized.user.user.auth {
            UserAuth::Token { token } => assert_eq!(token, "rotated-token"),
            _ => panic!("Expected token auth"),
        }
    }

    #[test]
    fn test_rotate_template_generation_token() {
        let user = User {
            name: "test-user".to_string(),
            user: UserDetails {
                auth: UserAuth::Token {
                    token: "token-xyz789:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
                },
            },
        };

        let cluster_name = "test-cluster";
        
        let template = format!(
            r#"# Rotate credentials for cluster: {}
# Update the authentication details below:

user:
  name: "{}"
  user:
{}
"#,
            cluster_name,
            user.name,
            match &user.user.auth {
                UserAuth::Token { token } => {
                    format!("    token: \"{}\"", token)
                }
                UserAuth::Certificate {
                    client_certificate_data,
                    client_key_data,
                } => {
                    format!(
                        "    client-certificate-data: \"{}\"\n    client-key-data: \"{}\"",
                        client_certificate_data, client_key_data
                    )
                }
            }
        );

        println!("Template:\n{}", template);

        // Try to parse it back
        #[derive(serde::Deserialize)]
        struct RotateConfig {
            user: User,
        }

        let parsed: Result<RotateConfig, _> = serde_yaml::from_str(&template);
        match parsed {
            Ok(config) => {
                println!("Successfully parsed!");
                match config.user.user.auth {
                    UserAuth::Token { token } => println!("Token: {}", token),
                    _ => println!("Not a token"),
                }
            }
            Err(e) => {
                println!("Failed to parse: {}", e);
                panic!("Template parsing failed");
            }
        }
    }

    #[test]
    fn test_multiple_token_users() {
        // Create a config with multiple token users to test mixed auth types
        let config = Config {
            clusters: vec![
                Cluster {
                    name: "dev-cluster".to_string(),
                    cluster: ClusterDetails {
                        server: "https://dev.example.com:6443".to_string(),
                        certificate_authority_data: None,
                    },
                },
                Cluster {
                    name: "prod-cluster".to_string(),
                    cluster: ClusterDetails {
                        server: "https://prod.example.com:6443".to_string(),
                        certificate_authority_data: None,
                    },
                },
                Cluster {
                    name: "staging-cluster".to_string(),
                    cluster: ClusterDetails {
                        server: "https://staging.example.com:6443".to_string(),
                        certificate_authority_data: Some("LS0tLS1CRUdJTi1DRVJUSUZJQ0FURS0tLS0t...".to_string()),
                    },
                },
            ],
            contexts: vec![
                Context {
                    name: "dev-cluster".to_string(),
                    context: ContextDetails {
                        cluster: "dev-cluster".to_string(),
                        user: "dev-user".to_string(),
                    },
                },
                Context {
                    name: "prod-cluster".to_string(),
                    context: ContextDetails {
                        cluster: "prod-cluster".to_string(),
                        user: "prod-user".to_string(),
                    },
                },
                Context {
                    name: "staging-cluster".to_string(),
                    context: ContextDetails {
                        cluster: "staging-cluster".to_string(),
                        user: "staging-user".to_string(),
                    },
                },
            ],
            users: vec![
                User {
                    name: "dev-user".to_string(),
                    user: UserDetails {
                        auth: UserAuth::Token {
                            token: "token-abc123:xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx".to_string(),
                        },
                    },
                },
                User {
                    name: "prod-user".to_string(),
                    user: UserDetails {
                        auth: UserAuth::Token {
                            token: "token-def456:yyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyy".to_string(),
                        },
                    },
                },
                User {
                    name: "staging-user".to_string(),
                    user: UserDetails {
                        auth: UserAuth::Certificate {
                            client_certificate_data: "LS0tLS1CRUdJTi1DRVJUSUZJQ0FURS0tLS0t...cert".to_string(),
                            client_key_data: "LS0tLS1CRUdJTi1FQyBQUklWQVRFIEtFWS0tLS0t...key".to_string(),
                        },
                    },
                },
            ],
            current_context: Some("prod-cluster".to_string()),
            preferences: None,
        };

        // Test finding users for each cluster
        for cluster in &config.clusters {
            println!("\n=== Testing cluster: {} ===", cluster.name);
            
            let context = config
                .contexts
                .iter()
                .find(|c| c.context.cluster == cluster.name)
                .cloned();
            
            if let Some(ctx) = &context {
                println!("Found context: {}, user reference: {}", ctx.name, ctx.context.user);
            }

            let user = context
                .as_ref()
                .and_then(|c| {
                    config
                        .users
                        .iter()
                        .find(|u| &u.name == &c.context.user)
                })
                .cloned();

            if let Some(u) = &user {
                println!("Found user: {}", u.name);
                match &u.user.auth {
                    UserAuth::Token { token } => println!("  Token: {}...", &token[..20]),
                    UserAuth::Certificate { .. } => println!("  Auth: Certificate"),
                }
            } else {
                println!("ERROR: User not found!");
            }
        }
    }

    #[test]
    fn test_rotate_credentials_after_navigation() {
        // This test reproduces the bug where rotate_credentials would show
        // credentials for the wrong cluster if you navigated with arrow keys
        let config = Config {
            clusters: vec![
                Cluster {
                    name: "cluster-1".to_string(),
                    cluster: ClusterDetails {
                        server: "https://server1:6443".to_string(),
                        certificate_authority_data: None,
                    },
                },
                Cluster {
                    name: "cluster-2".to_string(),
                    cluster: ClusterDetails {
                        server: "https://server2:6443".to_string(),
                        certificate_authority_data: None,
                    },
                },
            ],
            contexts: vec![
                Context {
                    name: "context-1".to_string(),
                    context: ContextDetails {
                        cluster: "cluster-1".to_string(),
                        user: "user-1".to_string(),
                    },
                },
                Context {
                    name: "context-2".to_string(),
                    context: ContextDetails {
                        cluster: "cluster-2".to_string(),
                        user: "user-2".to_string(),
                    },
                },
            ],
            users: vec![
                User {
                    name: "user-1".to_string(),
                    user: UserDetails {
                        auth: UserAuth::Token {
                            token: "token-for-user-1".to_string(),
                        },
                    },
                },
                User {
                    name: "user-2".to_string(),
                    user: UserDetails {
                        auth: UserAuth::Token {
                            token: "token-for-user-2".to_string(),
                        },
                    },
                },
            ],
            current_context: Some("context-1".to_string()),
            preferences: None,
        };

        let temp_dir = TempDir::new().unwrap();
        let kubeconfig_path = temp_dir.path().join("config");
        let mut app = App::new(config, kubeconfig_path);

        // Initially on cluster-1
        assert_eq!(app.selected_cluster.as_ref().unwrap().name, "cluster-1");
        
        // Get the user for cluster-1 
        let context1 = app.config.contexts.iter()
            .find(|c| c.context.cluster == "cluster-1").unwrap();
        let user1 = app.config.users.iter()
            .find(|u| u.name == context1.context.user).unwrap();
        match &user1.user.auth {
            UserAuth::Token { token } => assert_eq!(token, "token-for-user-1"),
            _ => panic!("Expected token"),
        }

        // Navigate to cluster-2
        app.next();
        assert_eq!(app.selected_cluster.as_ref().unwrap().name, "cluster-2");
        
        // Now the selected_cluster should be cluster-2
        // And if we were to call rotate_credentials, it should show credentials for user-2
        let context2 = app.config.contexts.iter()
            .find(|c| c.context.cluster == app.selected_cluster.as_ref().unwrap().name).unwrap();
        let user2 = app.config.users.iter()
            .find(|u| u.name == context2.context.user).unwrap();
        match &user2.user.auth {
            UserAuth::Token { token } => {
                assert_eq!(token, "token-for-user-2");
                println!("✓ After navigation, selected_cluster correctly points to cluster-2 with token-for-user-2");
            },
            _ => panic!("Expected token"),
        }
    }

    #[test]
    fn test_delete_selected_cluster() {
        let config = create_test_config();
        let temp_dir = TempDir::new().unwrap();
        let kubeconfig_path = temp_dir.path().join("config");

        // Write initial config
        let yaml_content = serde_yaml::to_string(&config).unwrap();
        fs::write(&kubeconfig_path, yaml_content).unwrap();

        let mut app = App::new(config, kubeconfig_path.clone());

        // Select first cluster and delete it
        app.cluster_list_state.select(Some(0));
        app.delete_selected();

        // Verify cluster was deleted
        assert_eq!(app.config.clusters.len(), 1);
        assert_eq!(app.config.clusters[0].name, "test-cluster-2");

        // Verify associated context was deleted
        assert_eq!(app.config.contexts.len(), 1);
        assert_eq!(app.config.contexts[0].name, "test-context-2");

        // Verify associated user was deleted (since not used by other contexts)
        assert_eq!(app.config.users.len(), 1);
        assert_eq!(app.config.users[0].name, "test-user-cert");
    }

    #[test]
    fn test_config_persistence() {
        let config = create_test_config();
        let temp_dir = TempDir::new().unwrap();
        let kubeconfig_path = temp_dir.path().join("config");

        // Write initial config to file
        let yaml_content = serde_yaml::to_string(&config).unwrap();
        fs::write(&kubeconfig_path, yaml_content).unwrap();

        // Create app and save config
        let app = App::new(config.clone(), kubeconfig_path.clone());
        app.save_config().unwrap();

        // Read back the config
        let saved_content = fs::read_to_string(&kubeconfig_path).unwrap();
        let loaded_config: Config = serde_yaml::from_str(&saved_content).unwrap();

        // Verify all data persisted correctly
        assert_eq!(loaded_config.clusters.len(), config.clusters.len());
        assert_eq!(loaded_config.users.len(), config.users.len());
        assert_eq!(loaded_config.contexts.len(), config.contexts.len());
        assert_eq!(loaded_config.current_context, config.current_context);
    }
}
