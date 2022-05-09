use eframe::{egui, emath::Vec2};
use std::{
    collections::HashMap,
    error::Error,
    env,
    fs::{self, File},
    io::Read,
    path::Path,
    iter,
    process,
};
use serde::{Serialize, Deserialize};
#[cfg(not(target_family = "windows"))]
use std::os::unix::fs::PermissionsExt;


#[derive(Serialize, Deserialize, Debug, Clone)]
struct AddonSpecification {
    required: Vec<String>,
    optional: Option<Vec<String>>,
    secondary: Option<String>,
}

type AddonMap = HashMap<String, AddonSpecification>;

fn main() -> Result<(), Box<dyn Error>> {
    let options = {
        let mut options = eframe::NativeOptions::default();
        options.initial_window_size = Some(Vec2 {x: 550., y: 300.});
        options
    };
    let quit_on_launch = env::args().any(|arg| arg.to_lowercase() == "--quit-on-launch");
    let addons = get_addons(None);
    match addons {
        Ok(addons) => {
            eframe::run_native("Doom 64 CE launcher", options,
                Box::new(move |_| Box::new(AddonManager::new(None, addons, quit_on_launch))));
        },
        Err(error) => {
            let message = format!("{:#?}", error);
            eframe::run_native("Doom 64 CE launcher", options,
                Box::new(|_| Box::new(ErrorMessage(message))));
        }
    }
}

fn get_addons(fname: Option<&str>) -> Result<AddonMap, Box<dyn Error>> {
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

const S_IXOTH: u32 = 0o1;
#[cfg(not(target_family = "windows"))]
fn is_executable(path: &impl AsRef<Path>) -> bool {
    // Linux/Unix uses a file permission bit
    let metadata = fs::metadata(path);
    match metadata {
        Ok(m) => {
            // let S_IXUSR = 0o100;
            // let S_IXGRP = 0o10;
            let mode = m.permissions().mode();
            (mode & (S_IXOTH)) != 0
        }
        Err(_) => false
    }
}
#[cfg(target_family = "windows")]
fn is_executable(path: &impl AsRef<Path>) -> bool {
    // Windows uses the .exe extension
    match path.extension() {
        Some(ext) => {ext.eq_ignore_ascii_case("exe")},
        None => false
    }
}

#[derive(Debug, Clone)]
struct AddonManager {
    builds: Box<[String]>,
    addons: AddonMap,
    primary_addons: Box<[String]>,
    secondary_addons: Box<[String]>,
    selected_primary_addon: usize,
    selected_secondary_addons: Box<[bool]>,
    selected_gzdoom_build: usize,
    quit_on_launch: bool,
}

impl AddonManager {
    pub fn new(gzdoom_build_glob_pattern: Option<&str>, addons: AddonMap, quit_on_launch: bool) -> AddonManager {
        let primary_addons: Box<[String]> = iter::once(String::from("None")).chain(addons.iter().filter(|(_name, addon)| {
            addon.secondary.is_none()
        }).map(|(name, _addon)| name.clone())).collect();
        let secondary_addons: Box<[String]> = addons.iter().filter(|(_name, addon)| {
            addon.secondary.is_some()
        }).map(|(name, _addon)| name.clone()).collect();
        let selected_secondary_addons: Box<[bool]> = Box::from_iter(secondary_addons.iter().map(|_| true));
        let pat = gzdoom_build_glob_pattern.unwrap_or(include_str!("gzdoom.glob"));
        let builds: Box<[String]> = match glob::glob(pat) {
            Ok(paths) => paths
                .filter_map(Result::ok)
                .filter_map(|p| {
                    match is_executable(&p) {
                        true => Some(p.to_str().unwrap_or("").to_string()),
                        false => None
                    }
                })
                .collect(),
            Err(_e) => Box::from([])
        };
        AddonManager {
            builds,
            primary_addons,
            secondary_addons,
            addons,
            selected_primary_addon: 0,
            selected_secondary_addons,
            selected_gzdoom_build: 0,
            quit_on_launch,
        }
    }
    fn gzdoom_build(&self) -> &str {
        self.builds.get(self.selected_gzdoom_build).map(String::as_str).unwrap_or("")
    }
    fn files_for_addon(&self, addon: Option<&AddonSpecification>) -> String {
        match addon {
            Some(addon) => {
                let mut files = String::new();
                for file in &addon.required {
                    files.push_str(file);
                    files.push_str("     ");
                }
                if let Some(optional) = &addon.optional {
                    for file in optional {
                        if File::open(file).is_ok() {
                            files.push_str(file);
                            files.push_str("     ");
                        }
                    }
                }
                files
            },
            None => String::from(""),
        }
    }
    fn primary_addon(&self) -> String {
        let name = self.primary_addons.get(self.selected_primary_addon).map(String::as_str).unwrap_or("");
        let addon = self.addons.get(name);
        self.files_for_addon(addon)
    }
    fn secondary_addons(&self) -> String {
        let addons: Vec<String> = self.secondary_addons.iter().zip(self.selected_secondary_addons.iter())
        .filter_map(|(addon, &selected)| if selected {Some(addon)} else {None})
        .cloned().collect();
        let mut addon_files = String::new();
        addons.iter().for_each(|addon| {
            let addon = self.addons.get(addon);
            addon_files.push_str(&self.files_for_addon(addon));
        });
        addon_files
    }
}

impl eframe::App for AddonManager {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ComboBox::from_label("GZDoom build")
            .selected_text(self.builds.get(self.selected_gzdoom_build)
                .unwrap_or(&String::from("None")))
            .width(400.).show_ui(ui, |ui| {
                self.builds.iter().enumerate().for_each(|(index, build)| {
                    ui.selectable_value(&mut self.selected_gzdoom_build, index, build);
                });
            });

            ui.separator();

            egui::ComboBox::from_label("Primary addon")
            .selected_text(self.primary_addons.get(self.selected_primary_addon)
                .unwrap_or(&String::from("None")))
            .width(400.).show_ui(ui, |ui| {
                self.primary_addons.iter().enumerate().for_each(|(index, addon)| {
                    ui.selectable_value(&mut self.selected_primary_addon, index, addon);
                });
            });

            ui.separator();

            egui::CollapsingHeader::new("Secondary addons")
            .default_open(true).show(ui, |ui| {
                self.selected_secondary_addons.iter_mut().zip(self.secondary_addons.iter())
                .for_each(|(selected, name)| {
                    ui.checkbox(selected, name);
                });
            });

            ui.separator();

            ui.horizontal(|ui| {
                if ui.button("Launch").clicked() {
                    let gzdoom_build = self.gzdoom_build();
                    let primary_addon = self.primary_addon();
                    let secondary_addons = self.secondary_addons();
                    println!("{}", gzdoom_build);
                    print!("{}", primary_addon);
                    println!("{}", secondary_addons);
                    if self.quit_on_launch {
                        process::exit(0);
                    }
                }

                if ui.button("Exit").clicked() {
                    process::exit(0);
                }
            });
        });
    }
}

struct ErrorMessage(String);
impl eframe::App for ErrorMessage {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Error!");
            ui.label(&self.0);
            if ui.button("Exit").clicked() {
                process::exit(1);
            }
        });
    }
}
