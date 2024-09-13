use std::{ fs, env, path::PathBuf, error::Error };
use crate::types::Config;
use serde_yaml::Value;

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
            serde_yaml::to_value(&config.clusters)?
        );
        mapping.insert(Value::String("users".to_string()), serde_yaml::to_value(&config.users)?);
        mapping.insert(
            Value::String("contexts".to_string()),
            serde_yaml::to_value(&config.contexts)?
        );

        if !mapping.contains_key("apiVersion") {
            mapping.insert(
                Value::String("apiVersion".to_string()),
                Value::String("v1".to_string())
            );
        }
        if !mapping.contains_key("kind") {
            mapping.insert(Value::String("kind".to_string()), Value::String("Config".to_string()));
        }
    }

    let yaml_content = serde_yaml::to_string(&existing_yaml)?;
    fs::write(path, yaml_content)?;
    Ok(())
}
