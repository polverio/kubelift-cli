use kubelift::Appliance;
use kubelift::KubeLiftConfig;

use anyhow::{Ok, Result};
use chrono::{SecondsFormat, Utc};
use core::time::Duration;
use std::fs;

#[cfg(not(target_os = "windows"))]
use std::os::unix::prelude::PermissionsExt;

use random_string::generate;
use serde_json::{self};
use serde_yaml::{self};
use spinners::{Spinner, Spinners};
use std::{fs::copy, path::Path, thread::sleep};
use xshell::{cmd, Shell};

#[derive(Clone, Debug)]
pub struct KubeLift;

fn kubelift_config() -> KubeLiftConfig {
    let f = std::fs::File::open("kubelift.yml").expect("Could not open file.");
    let config: KubeLiftConfig = serde_yaml::from_reader(f).expect("Failed to read configuration.");
    return config;
}

fn local_kubeconfig_exists() -> bool {
    return Path::new("./.kubelift/kubeconfig").exists();
}

fn kubelift_config_file_exists() -> bool {
    return Path::new("kubelift.yml").exists();
}

fn generate_new_instance_id() -> String {
    let charset: String = "abcdefghjklmnpqrstuvwxyz123456789".to_string();
    return format!("{}", generate(5, charset).to_string());
}

#[cfg(not(target_os = "windows"))]
const PLATFORM_SPECIFIC_AZ_COMMAND: &str = "az";

#[cfg(target_os = "windows")]
const PLATFORM_SPECIFIC_AZ_COMMAND: &str = "az.cmd";

/// Initializes the kubelift.yml file
fn init() -> Result<()> {
    let mut sp = Spinner::new(
        Spinners::Dots,
        r#" Initializing KubeLift config file"#.into(),
    );
    let default_yaml = r#"---
    cloud: AzurePublic

    options:
        location: westeurope
        size: Standard_B4ms
        image: MicrosoftCBLMariner:cbl-mariner:cbl-mariner-2-gen2:latest
        tags: KUBE_CHANNEL=stable
    "#;

    let config: KubeLiftConfig = serde_yaml::from_str(default_yaml).unwrap();
    let f = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .open("kubelift.yml")
        .expect("Couldn't open file");
    serde_yaml::to_writer(f, &config).unwrap();

    sp.stop_and_persist(
        " \x1b[32m✔\x1b[0m",
        format!("Initialized KubeLift config file: {}", "kubelift.yml").into(),
    );
    Ok(())
}

/// Checking software is in the path
fn preflight() -> Result<()> {
    Ok(())
}

/// All the logic that creates the instance goes in here
fn up() -> Result<()> {
    if !kubelift_config_file_exists() {
        init().unwrap();
    }

    let sh = Shell::new()?;
    let instance_id = generate_new_instance_id();
    let kubelift_config = kubelift_config();
    let location = kubelift_config.options.location;
    let size = kubelift_config.options.size;
    let image: String = kubelift_config.options.image;
    let tags: String = kubelift_config.options.tags;

    // Provision Resource Group
    let mut sp = Spinner::new(
        Spinners::Dots,
        format!(
            "Creating cluster kubelift-{} in location {}",
            instance_id, location
        )
        .into(),
    );
    let resource_group = cmd!(
        sh,
        "{PLATFORM_SPECIFIC_AZ_COMMAND} group create -n kubelift-{instance_id} -l {location} -o json")
    .quiet()
    .read()?;
    sh.write_file("./.kubelift/resource_group.json", &resource_group)
        .unwrap();
    sp.stop_with_symbol(" \x1b[32m✔\x1b[0m");

    // Create the VM
    sp = Spinner::with_timer(
        Spinners::Dots,
        "Creating appliance from Azure Marketplace".into(),
    );
    let vm: String = cmd!(sh, "{PLATFORM_SPECIFIC_AZ_COMMAND} vm create --resource-group kubelift-{instance_id} --name {instance_id} --image {image} --admin-username kubelift --generate-ssh-keys --size {size} --public-ip-sku Standard --tags {tags}")
        .quiet()
        .ignore_stderr()
        .read()?;
    sh.write_file("./.kubelift/vm.json", &vm).unwrap();
    sp.stop_with_symbol(" \x1b[32m✔\x1b[0m");

    // Ensure access to port 6443
    sp = Spinner::new(
        Spinners::Dots,
        "Granting access to the Kubernetes api-server on port 6443".into(),
    );
    let _nsg: String = cmd!(
        sh,
        "{PLATFORM_SPECIFIC_AZ_COMMAND} vm open-port --resource-group kubelift-{instance_id} --name {instance_id} --port '*'"
    )
    .quiet()
    .ignore_stderr()
    .read()?;
    sp.stop_with_symbol(" \x1b[32m✔\x1b[0m");

    // Get the public IP of the instance
    sp = Spinner::new(Spinners::Dots, "Getting IP addresses from VM".into());
    let vm_info: serde_json::Value = serde_json::from_str(&vm)?;
    let private_ip = vm_info["privateIpAddress"].as_str().unwrap();
    let public_ip = vm_info["publicIpAddress"].as_str().unwrap();

    sp.stop_and_persist(
        " \x1b[32m✔\x1b[0m",
        format!(
            "Appliance is publicly accessible on IP: {} (VNET-bound IP: {})",
            public_ip, private_ip
        )
        .into(),
    );

    sp = Spinner::new(Spinners::Dots, "Deploying KubeLift".into());
    let _running = cmd!(sh, "ssh -tt -o 'StrictHostKeyChecking no' kubelift@{public_ip} 'curl -L0 https://raw.githubusercontent.com/polverio/releases/main/azure/prereqs.sh > /tmp/kubelift.sh && sudo sh /tmp/kubelift.sh'")
        .ignore_stderr()
        .read()?;
    sp.stop_and_persist(
        " \x1b[32m✔\x1b[0m",
        "Completed deployment of KubeLift Solo".into(),
    );

    // Retrieve the kubeconfig via SSH
    sp = Spinner::new(Spinners::Dots, "Awaiting appliance configuration".into());
    let kubeconfig = cmd!(sh, "ssh -tt -o 'StrictHostKeyChecking no' kubelift@{public_ip} 'sudo cat /etc/kubernetes/admin.conf'" )
        .quiet()
        .ignore_stderr()
        .read()?;
    sp.stop_and_persist(
        " \x1b[32m✔\x1b[0m",
        "Retrieved configuration from appliance".into(),
    );

    // Patching kubeconfig with public IP address
    sp = Spinner::new(
        Spinners::Dots,
        "Modifying kubeconfig to point at public IP of appliance".into(),
    );
    let updated_kubeconfig = &kubeconfig.replace(private_ip, public_ip);
    sh.write_file("./.kubelift/kubeconfig", &updated_kubeconfig)
        .unwrap();

    #[cfg(not(target_os = "windows"))]
    {
        let mut perms = fs::metadata("./.kubelift/kubeconfig")?.permissions();
        perms.set_mode(0o600);
        fs::set_permissions("./.kubelift/kubeconfig", perms)?;
    }

    sp.stop_and_persist(
        " \x1b[32m✔\x1b[0m",
        "Modified local kubeconfig to point at public IP of appliance".into(),
    );

    // Switching context to local kubeconfig
    sp = Spinner::new(
        Spinners::Dots,
        "Switching Kubernetes context to this instance".into(),
    );
    switch().unwrap();
    sleep(Duration::from_millis(250));
    sp.stop_and_persist(" \x1b[32m✔\x1b[0m", "KubeLift instance is ready".into());

    Ok(())
}

fn switch() -> Result<()> {
    // println!("\n📁 [switch]");
    let mut sp = Spinner::new(Spinners::Dots, "Switching to remote kubeconfig".into());
    sleep(Duration::from_millis(250));

    let current_date = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
    let kubeconfig_location_global = dirs::home_dir()
        .map(|d| d.join(".kube").join("config"))
        .unwrap()
        .to_string_lossy()
        .to_string();

    let kubeconfig_location_backup = dirs::home_dir()
        .map(|d| {
            d.join(".kube")
                .join(format!("config.{}.bak", &current_date))
        })
        .unwrap()
        .to_string_lossy()
        .to_string();

    if local_kubeconfig_exists() {
        let kube_path = Path::new(&kubeconfig_location_global).parent().unwrap(); 
        if Path::exists(kube_path) {
            copy(&kubeconfig_location_global, &kubeconfig_location_backup).unwrap();
        } 
        else {
            fs::create_dir_all(kube_path).unwrap();
        }
        copy(".kubelift/kubeconfig", &kubeconfig_location_global).unwrap();
    }

    sp.stop_and_persist(
        " \x1b[32m✔\x1b[0m",
        "Set kubectl context to \"admin@kubernetes\"".into(),
    );

    Ok(())
}

fn down() -> Result<()> {
    let resource_group_file = Path::new("./.kubelift/resource_group.json");
    if resource_group_file.exists() {
        let sh = Shell::new()?;

        let resource_group = sh.read_file(resource_group_file).unwrap();
        let resource_group_info: serde_json::Value = serde_json::from_str(&resource_group)?;
        let full_instance_id = resource_group_info["name"].as_str().unwrap();

        let mut sp = Spinner::new(
            Spinners::Monkey,
            format!("Deleting KubeLift instance: {}", full_instance_id).into(),
        );

        let _resource_group = cmd!(sh, "{PLATFORM_SPECIFIC_AZ_COMMAND} group delete -n {full_instance_id} --force-deletion-types Microsoft.Compute/virtualMachines --no-wait --yes")
        .quiet()
        .read()?;

        sp.stop_and_persist("🏁", format!("Deletion of cluster {} will continue in the background. Thanks for using KubeLift!",full_instance_id).to_string());

        clean().unwrap();
    } else {
        println!("No KubeLift appliance metadata found in current location. Exiting.")
    }

    Ok(())
}

fn clean() -> Result<()> {
    let mut sp = Spinner::new(Spinners::Dots, "Cleaning up config files".into());
    let sh = Shell::new()?;
    sh.remove_path("./.kubelift").unwrap();
    sh.remove_path("./kubelift.yml").unwrap();
    sp.stop_and_persist(" \x1b[32m✔\x1b[0m", "Cleaned up config files".into());

    Ok(())
}

impl Appliance for KubeLift {
    fn smoke(&self) {
        println!("This is the Azure plugin for KubeLift!")
    }

    fn init(&self) {
        preflight().unwrap();
        init().unwrap();
    }

    fn up(&self) {
        up().unwrap();
    }

    fn down(&self) {
        down().unwrap();
    }

    fn clean(&self) {
        clean().unwrap();
    }

    fn switch(&self) {
        switch().unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;

    #[test]
    fn test_kubelift_config_file_exists() {
        let file_exists = kubelift_config_file_exists();
        assert_eq!(file_exists, Path::new("kubelift.yml").exists());
    }

    #[test]
    fn test_local_kubeconfig_exists() {
        let file_exists = local_kubeconfig_exists();
        assert_eq!(file_exists, Path::new("./.kubelift/kubeconfig").exists());
    }

    #[test]
    fn test_generate_new_instance_id() {
        let instance_id = generate_new_instance_id();
        assert_eq!(instance_id.len(), 5);
        assert!(instance_id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_digit(10)));
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
        assert_eq!(config.options.tags, "KUBE_CHANNEL=stable");
    }
}
