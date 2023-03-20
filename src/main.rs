/* KubeLift (c) 2023 Polverio Ltd */
extern crate kubelift;
use clap::{Parser, Subcommand};
use easy_di::{Container, ServiceProvider};
use kubelift::Appliance;
use std::env;
use std::sync::Arc;
use std::time::Instant;

mod clouds;

#[derive(Debug, Parser)]
#[command(author, version, about, long_about = "", arg_required_else_help = !env::var("FORCE_UI").is_ok())]
pub struct KubeLiftCli {
    #[command(subcommand)]
    command: Option<KubeLiftCommands>,

    /// Send additional debug information to <stdout>
    #[arg(short, long, default_value_t = false, global = true, required = false)]
    debug: bool,

    /// Future support for different cloud types. Default is Azure.
    #[arg(short, long, global = true, required = false)]
    cloud: Option<String>,

    /// Automatically accepts the Azure Marketplace license terms.
    #[arg(short, long, default_value_t = false, global = true, required = false)]
    accept_azure_marketplace_terms: bool,
}

#[derive(Subcommand, Debug)]
pub enum KubeLiftCommands {
    /// Initializes the KubeLift configuration file
    Init {},
    /// Creates and starts an instance of KubeLift
    Up {},
    /// Stops and deletes an instance of KubeLift
    Down {},
    /// Cleans up instance-related data and kubelift.yml in current directory
    Clean {},
    /// Switches your local Kubernetes configuration to point to the current appliance
    Switch {},
}

fn main() {
    // TODO: select cloud type properly - for now we're forcing use of Azure
    let cloud_type = clouds::azure::KubeLift;
    let mut container = Container::new();

    let appliance: Arc<dyn Appliance + Sync + Send> = Arc::new(cloud_type);
    container.inject(appliance);

    let instance = container.find::<Arc<dyn Appliance + Sync + Send>>();

    let now = Instant::now();
    let this = KubeLiftCli::parse();

    match &this.command {
        Some(KubeLiftCommands::Up{ .. }) => {
            instance.unwrap().up();
        }

        Some(KubeLiftCommands::Down {}) => {
            instance.unwrap().down();
        }

        Some(KubeLiftCommands::Init {}) => {
            instance.unwrap().init();
        }

        Some(KubeLiftCommands::Clean {}) => {
            instance.unwrap().clean();
        }

        Some(KubeLiftCommands::Switch {}) => {
            instance.unwrap().switch();
        }

        None => {}
    }

    println!("\nGot a question or found a bug? Find us at https://github.com/polverio/kubelift-cli");

    let elapsed = now.elapsed();
    println!("\n⏱️  Command took: {:.2?} 💨", elapsed);
}
