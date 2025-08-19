use crate::config;
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
}

impl App {
    pub fn new(config: Config, kubeconfig_path: PathBuf) -> Self {
        let mut app = Self {
            config,
            cluster_list_state: ListState::default(),
            selected_cluster: None,
            needs_redraw: false,
            kubeconfig_path,
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
    }

    pub fn previous(&mut self) {
        let i = match self.cluster_list_state.selected() {
            Some(i) => (i + self.config.clusters.len() - 1) % self.config.clusters.len(),
            None => 0,
        };
        self.cluster_list_state.select(Some(i));
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

    pub fn save_config(&self) -> Result<(), Box<dyn std::error::Error>> {
        config::save_config(&self.config, &self.kubeconfig_path)
    }
}
