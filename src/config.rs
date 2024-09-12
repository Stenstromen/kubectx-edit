use std::{fs, env, path::PathBuf, error::Error};
use crate::types::Config;
use serde_yaml;

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
    let yaml_content = serde_yaml::to_string(config)?;
    fs::write(path, yaml_content)?;
    Ok(())
}
