use serde::{Deserialize, Serialize};

pub trait Appliance {
    fn smoke(&self);
    fn init(&self);
    fn up(&self);
    fn down(&self);
    fn clean(&self);
    fn switch(&self);
}

#[derive(Serialize, Deserialize)]
pub struct KubeLiftConfig {
    pub cloud: String,
    pub options: KubeLiftConfigOptions,
}

#[derive(Serialize, Deserialize)]
pub struct KubeLiftConfigOptions {
    pub image: String,
    pub location: String,
    pub size: String,
    pub tags: String
}
