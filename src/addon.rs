use std::{collections::HashMap, error::Error, fs::File, io::Read};
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AddonSpecification {
    pub required: Vec<String>,
    pub optional: Option<Vec<String>>,
    pub secondary: Option<String>,
}

pub type AddonMap = HashMap<String, AddonSpecification>;

pub fn get_addons(fname: Option<&str>) -> Result<AddonMap, Box<dyn Error>> {
    let contents = {
        let mut file = File::open(fname.unwrap_or("addons.yml"))?;
        let mut s = String::new();
        file.read_to_string(&mut s)?;
        s
    };

    #[derive(Serialize, Deserialize, Debug, Clone)]
    struct Addons {
        addons: AddonMap,
    }

    let addons: Addons = serde_yaml::from_str(&contents)?;
    let addons: AddonMap = addons.addons.into_iter()
        .filter(|(name, entry)| {
        name.to_lowercase() != "none" &&
        entry.required.iter().all(|req_file| File::open(req_file).is_ok())
    }).collect();
    Ok(addons)
}
