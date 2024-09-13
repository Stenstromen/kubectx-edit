use crate::types::{ Config, Cluster, TempConfig };
use crate::config;
use ratatui::widgets::ListState;
use std::{ io, env, path::PathBuf, process::Command };
use tempfile::NamedTempFile;
use crossterm::{ execute, terminal::{ EnterAlternateScreen, LeaveAlternateScreen } };

pub struct App {
    pub config: Config,
    pub cluster_list_state: ListState,
    pub selected_cluster: Option<Cluster>,
    pub show_menu: bool,
    pub menu_state: ListState,
    pub needs_redraw: bool,
    pub kubeconfig_path: PathBuf,
}

impl App {
    pub fn new(config: Config, kubeconfig_path: PathBuf) -> Self {
        Self {
            config,
            cluster_list_state: ListState::default(),
            selected_cluster: None,
            show_menu: false,
            menu_state: ListState::default(),
            needs_redraw: false,
            kubeconfig_path,
        }
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

    pub fn toggle_menu(&mut self) {
        self.show_menu = !self.show_menu;
        self.menu_state.select(Some(0));
    }

    pub fn delete_selected(&mut self) {
        if let Some(selected) = self.cluster_list_state.selected() {
            self.config.clusters.remove(selected);
            self.cluster_list_state.select(Some(selected.saturating_sub(1)));
            self.save_config().expect("Failed to save config");
            self.needs_redraw = true;
        }
    }

    pub fn edit_selected(&mut self) -> io::Result<()> {
        if let Some(cluster) = &self.selected_cluster {
            let editor = env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());
            let temp_file = NamedTempFile::new()?;

            let context = self.config.contexts
                .iter()
                .find(|c| &c.name == &cluster.name)
                .cloned();
            let user = context
                .as_ref()
                .and_then(|c| self.config.users.iter().find(|u| &u.name == &c.context.user))
                .cloned();

            let temp_config = TempConfig {
                cluster: cluster.clone(),
                context,
                user,
            };

            serde_yaml
                ::to_writer(&temp_file, &temp_config)
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
                        if
                            let Some(index) = self.config.clusters
                                .iter()
                                .position(|c| c.name == cluster.name)
                        {
                            self.config.clusters[index] = edited_config.cluster;
                            self.selected_cluster = Some(self.config.clusters[index].clone());
                        }
                        if let Some(context) = edited_config.context {
                            if
                                let Some(index) = self.config.contexts
                                    .iter()
                                    .position(|c| c.name == context.name)
                            {
                                self.config.contexts[index] = context;
                            } else {
                                self.config.contexts.push(context);
                            }
                        }
                        if let Some(user) = edited_config.user {
                            if
                                let Some(index) = self.config.users
                                    .iter()
                                    .position(|u| u.name == user.name)
                            {
                                self.config.users[index] = user;
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
                        let yaml_content = serde_yaml
                            ::to_string(&updated_config)
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
        self.show_menu = false;
        self.menu_state = ListState::default();
        self.needs_redraw = true;
        Ok(())
    }

    pub fn add_new_kubeconfig(&mut self) -> io::Result<()> {
        let editor = env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());
        let temp_file = NamedTempFile::new()?;

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
            match serde_yaml::from_str::<Config>(&edited_content) {
                Ok(new_config) => {
                    self.config.clusters.extend(new_config.clusters);
                    self.config.users.extend(new_config.users);
                    self.config.contexts.extend(new_config.contexts);

                    let updated_config = Config {
                        clusters: self.config.clusters.clone(),
                        users: self.config.users.clone(),
                        contexts: self.config.contexts.clone(),
                        current_context: self.config.current_context.clone(),
                        preferences: self.config.preferences.clone(),
                    };
                    let yaml_content = serde_yaml
                        ::to_string(&updated_config)
                        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
                    std::fs::write(&self.kubeconfig_path, yaml_content)?;
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

    pub fn menu_next(&mut self) {
        let len = 2;
        let i = match self.menu_state.selected() {
            Some(i) => (i + 1) % len,
            None => 0,
        };
        self.menu_state.select(Some(i));
    }

    pub fn menu_previous(&mut self) {
        let len = 2;
        let i = match self.menu_state.selected() {
            Some(i) => (i + len - 1) % len,
            None => len - 1,
        };
        self.menu_state.select(Some(i));
    }
}
