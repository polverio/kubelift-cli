use crate::KubeLiftCli;
use kubelift::Appliance;
use kubelift::KubeLiftConfig;

use anyhow::{Ok, Result};
use chrono::{SecondsFormat, Utc};
use clap::Parser;
use core::time::Duration;
use minreq;
use question::{Answer, Question};
use random_string::generate;
use serde_json::{self};
use serde_yaml::{self};
use spinners::{Spinner, Spinners};
use std::process::exit;
use std::{fs::copy, path::Path, thread::sleep};
use xshell::{cmd, Shell};

// This is great

#[derive(Clone, Debug)]
pub struct KubeLift;

fn kubelift_config() -> KubeLiftConfig {
    let f = std::fs::File::open("kubelift.yml").expect("Could not open file.");
    let config: KubeLiftConfig = serde_yaml::from_reader(f).expect("Failed to read configuration.");
    return config;
}

fn kubeconfig_exists() -> bool {
    return Path::new("./.kubelift/kubeconfig").exists();
}

fn kubelift_config_file_exists() -> bool {
    return Path::new("kubelift.yml").exists();
}

fn generate_new_instance_id() -> String {
    let charset: String = "abcdefghjklmnpqrstuvwxyz123456789".to_string();
    return format!("{}", generate(5, charset).to_string());
}

fn azure_marketplace_terms_have_been_accepted() -> bool {
    let cli_config = config();
    return cli_config.accept_azure_marketplace_terms;
}

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
        image: polverio:kubelift:solo:latest
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
        format!("Initialized KubeLift config file: {}", "kubelift.yml").into()
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

    // cmd!(sh, "az account list --query '[?isDefault]'")
    //     .quiet()
    //     .run()?;

    let mut sp = Spinner::new(
        Spinners::Dots,
        format!("Getting Azure Marketplace status for {}", image).into(),
    );

    let terms_show = cmd!(sh, "az vm image terms show --urn {image}")
        .quiet()
        .read()?;

    let terms: serde_json::Value = serde_json::from_str(&terms_show)?;
    let terms_accepted = terms["accepted"].as_bool().unwrap();
    let marketplace_terms_link = terms["marketplaceTermsLink"].as_str().unwrap();
    sp.stop_with_symbol(" \x1b[32m✔\x1b[0m");

    if terms_accepted == false {
        let response = minreq::get(marketplace_terms_link)
            .with_timeout(10)
            .send()?;
        println!(
            "\n\nAZURE MARKETPLACE TERMS (please read)\n\n{}\n",
            response.as_str()?
        );

        let answer = Question::new("Accept the Azure Marketplace terms?")
            .default(Answer::YES)
            .show_defaults()
            .confirm();

        if answer == Answer::YES || azure_marketplace_terms_have_been_accepted() {
            // Accept terms
            sp = Spinner::new(
                Spinners::Dots,
                format!("Accepting Azure Marketplace terms for {image}").into(),
            );
            let _terms_accept = cmd!(sh, "az vm image termsQQQ accept --urn {image}")
                .quiet()
                .ignore_stderr()
                .read()?;
            sp.stop_with_symbol(" \x1b[32m✔\x1b[0m");
        } else {
            println!("Aborting...");
            exit(1);
        }
    }   

    // Provision Resource Group
    sp = Spinner::new(
        Spinners::Dots,
        format!(
            "Creating cluster kubelift-{} in location {}",
            instance_id, location
        )
        .into(),
    );
    let resource_group = cmd!(
        sh,
        "az group create -n kubelift-{instance_id} -l {location} -o json"
    )
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
    let vm: String = cmd!(sh, "az vm create --resource-group kubelift-{instance_id} --name {instance_id} --image {image} --admin-username kubelift --generate-ssh-keys --size {size} --public-ip-sku Standard")
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
        "az vm open-port --resource-group kubelift-{instance_id} --name {instance_id} --port 6443"
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

    // Retrieve the kubeconfig via SSH
    sp = Spinner::new(Spinners::Dots, "Awaiting appliance configuration".into());
    let kubeconfig = cmd!(sh, "ssh -tt -o 'StrictHostKeyChecking no' kubelift@{public_ip} 'while [ ! -f /etc/kubernetes/admin.conf ]; do sleep 5; done; sudo cat /etc/kubernetes/admin.conf'" )
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

    if kubeconfig_exists() {
        copy(&kubeconfig_location_global, &kubeconfig_location_backup).unwrap();
        // println!(
        //     "\n    Original kubeconfig copied to {}",
        //     &kubeconfig_location_backup
        // );
        // println!(
        //     "    Copying appliance kubeconfig to {}",
        //     &kubeconfig_location_global
        // );
        copy(".kubelift/kubeconfig", &kubeconfig_location_global).unwrap();
    }

    sp.stop_and_persist(
        " \x1b[32m✔\x1b[0m",
        "Set kubectl context to \"admin@kubernetes\"".into(),
    );

    Ok(())
}

fn down() -> Result<()> {
    // println!("\n🗑️  [down]");

    let sh = Shell::new()?;

    let resource_group = sh.read_file("./.kubelift/resource_group.json").unwrap();
    let resource_group_info: serde_json::Value = serde_json::from_str(&resource_group)?;
    let full_instance_id = resource_group_info["name"].as_str().unwrap();

    let mut sp = Spinner::new(
        Spinners::Monkey,
        format!("Deleting KubeLift instance: {}", full_instance_id).into(),
    );

    let _resource_group = cmd!(sh, "az group delete -n {full_instance_id} --force-deletion-types Microsoft.Compute/virtualMachines --no-wait --yes")
        .quiet()
        .read()?;

    sp.stop_and_persist("🏁", format!("Deletion of cluster {} will continue in the background. Thanks for using KubeLift!",full_instance_id).to_string());

    clean().unwrap();

    Ok(())
}

fn clean() -> Result<()> {
    // println!("\n🧽 [clean]");
    let mut sp = Spinner::new(Spinners::Dots, "Cleaning up config files".into());
    let sh = Shell::new()?;
    sh.remove_path("./.kubelift").unwrap();
    sp.stop_and_persist(" \x1b[32m✔\x1b[0m", "Cleaned up config files".into());

    Ok(())
}

fn config() -> KubeLiftCli {
    return KubeLiftCli::parse();
}

impl Appliance for KubeLift {
    fn smoke(&self) {
        println!("This is the Azure plugin for KubeLift!")
    }

    fn up(&self) {
        up().unwrap();
    }

    fn init(&self) {
        preflight().unwrap();
        init().unwrap();
    }

    fn down(&self) {
        down().unwrap();
    }

    fn switch(&self) {
        switch().unwrap();
    }

    fn clean(&self) {
        clean().unwrap();
    }
}
