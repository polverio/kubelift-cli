use std::fs;
use std::path::Path;

use kubelift::clouds::azure::{KubeLift, kubelift_config, kubeconfig_exists, kubelift_config_file_exists, generate_new_instance_id};

#[test]
fn test_kubelift_config_file_exists() {
    let file_exists = kubelift::kubelift_config_file_exists();
    assert_eq!(file_exists, Path::new("kubelift.yml").exists());
}

#[test]
fn test_kubeconfig_exists() {
    let file_exists = kubelift::kubeconfig_exists();
    assert_eq!(file_exists, Path::new("./.kubelift/kubeconfig").exists());
}

#[test]
fn test_generate_new_instance_id() {
    let instance_id = kubelift::generate_new_instance_id();
    assert_eq!(instance_id.len(), 5);
    assert!(instance_id.chars().all(|c| c.is_ascii_lowercase() || c.is_digit(10)));
}

#[test]
fn test_init() {
    let kubelift = KubeLift;
    kubelift.init();

    let config_path = Path::new("kubelift.yml");
    assert!(config_path.exists());

    let contents = fs::read_to_string(config_path).unwrap();
    let config: KubeLiftConfig = serde_yaml::from_str(&contents).unwrap();

    assert_eq!(config.cloud, "AzurePublic");
    assert_eq!(config.options.location, "westeurope");
    assert_eq!(config.options.size, "Standard_B4ms");
    assert_eq!(
        config.options.image,
        "MicrosoftCBLMariner:cbl-mariner:cbl-mariner-2-gen2:latest"
    );
    assert_eq!(config.options.tags, "kubelift-instance=true");
}
