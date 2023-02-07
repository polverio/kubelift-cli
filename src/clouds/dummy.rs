use kubelift::Appliance;

#[derive(Clone)]
#[derive(Debug)]
pub struct KubeLift;

impl Appliance for KubeLift {
    fn smoke(&self) {
        println!("This is the dummy plugin for KubeLift!")
    }
}