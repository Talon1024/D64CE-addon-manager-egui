use serde::{Deserialize, Serialize};
use std::{collections::HashMap, env, error::Error, fs::File, iter, process::Command};

mod addon;
mod apps;
mod checks;
mod cmdlineparse;
mod command;

use addon::{AddonMap, AddonSpecification};
use apps::error::ErrorMessage;
use checks::*;
use command::*;
use eframe::{App, AppCreator, Frame, HardwareAcceleration, NativeOptions, Storage, Theme};

#[derive(Debug, Clone, Default)]
struct AppOptions {
    quit_on_launch: bool,
    gzdoom_glob: Option<String>,
    iwad_glob: Option<String>,
}

fn main() -> Result<(), Box<dyn Error>> {
    let app_options = AppOptions {
        quit_on_launch: env::args().any(|arg| arg == "--quit-on-launch"),
        gzdoom_glob: env::args()
            .skip_while(|arg| arg != "--gzdoom-glob")
            .skip(1)
            .next(),
        iwad_glob: env::args()
            .skip_while(|arg| arg != "--iwad-glob")
            .skip(1)
            .next(),
        ..Default::default()
    };
    let addons: Result<HashMap<String, AddonSpecification>, Box<dyn Error>> =
        addon::get_addons(None);
    let app: AppCreator = Box::new(|cc| -> Box<dyn App> {
        let data = cc.storage.map(Persistence::from);
        match addons {
            Ok(addons) => Box::new(AddonManager::new(addons, app_options, data)),
            Err(error) => {
                let message = format!("{:#?}", error);
                Box::new(ErrorMessage::from(message))
            }
        }
    });
    let native_options = NativeOptions {
        always_on_top: false,
        maximized: true,
        decorated: true,
        fullscreen: true,
        drag_and_drop_support: false,
        icon_data: None,
        initial_window_pos: None,
        initial_window_size: None,
        min_window_size: None,
        max_window_size: None,
        resizable: true,
        transparent: false,
        mouse_passthrough: false,
        active: true,
        vsync: false,
        multisampling: 0,
        depth_buffer: 0,
        stencil_buffer: 0,
        hardware_acceleration: HardwareAcceleration::Preferred,
        renderer: Default::default(),
        follow_system_theme: false,
        default_theme: Theme::Dark,
        run_and_return: false,
        event_loop_builder: None,
        shader_version: None,
        centered: true,
        app_id: Some(String::from("Talon1024.Talauncher")),
    };
    eframe::run_native("Talauncher", native_options, app).map_err(Box::from)
}

#[derive(Debug, Clone)]
enum GZDoomBuildSelection {
    Single,           // Hide GZDoom build selector
    ListIndex(usize), // Show a drop-down list
    FullPath(String), // Show text box and "Browse" button
}

impl Default for GZDoomBuildSelection {
    fn default() -> Self {
        GZDoomBuildSelection::FullPath(String::new())
    }
}

#[derive(Debug, Clone, Default)]
struct AddonManager {
    builds: Box<[String]>,
    iwads: Box<[String]>,
    addons: AddonMap,
    primary_addons: Box<[String]>,
    secondary_addons: Box<[String]>,
    selected_primary_addon: usize,
    selected_secondary_addons: Box<[bool]>,
    selected_gzdoom_build: GZDoomBuildSelection,
    selected_iwad: GZDoomBuildSelection,
    quit_on_launch: bool,
    popup: Option<String>,
    quit: bool,
    exargs: String,
    config: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Persistence {
    gzdoom_build: Option<String>,
    primary_addon: Option<String>,
    secondary_addons: Option<HashMap<String, bool>>,
    exargs: Option<String>,
    config: Option<String>,
    iwad: Option<String>,
}

impl Persistence {
    fn save(&self, storage: &mut dyn Storage) {
        if let Some(ref gzdoom_build) = self.gzdoom_build {
            storage.set_string("gzdoom_build", gzdoom_build.clone());
        }
        if let Some(ref primary_addon) = self.primary_addon {
            storage.set_string("primary_addon", primary_addon.clone());
        }
        if let Some(ref secondary_addons) = self.secondary_addons {
            let secondary_addons = serde_yaml::to_string(secondary_addons).unwrap();
            storage.set_string("secondary_addons", secondary_addons);
        }
        if let Some(ref exargs) = self.exargs {
            storage.set_string("exargs", exargs.clone());
        }
        if let Some(ref config) = self.config {
            storage.set_string("config", config.clone());
        }
        if let Some(ref iwad) = self.iwad {
            storage.set_string("iwad", iwad.clone());
        }
    }
}

impl From<&AddonManager> for Persistence {
    fn from(v: &AddonManager) -> Self {
        Persistence {
            gzdoom_build: Some(String::from(v.gzdoom_build())),
            primary_addon: match v.selected_primary_addon {
                0 => None,
                _ => Some(v.primary_addons[v.selected_primary_addon].clone()),
            },
            secondary_addons: match v.secondary_addons.len() {
                0 => None,
                _ => Some(
                    v.secondary_addons
                        .iter()
                        .cloned()
                        .zip(v.selected_secondary_addons.iter().cloned())
                        .collect(),
                ),
            },
            exargs: match v.exargs.len() {
                0 => None,
                _ => Some(v.exargs.clone()),
            },
            config: match v.config.len() {
                0 => None,
                _ => Some(v.config.clone()),
            },
            iwad: Some(
                match v.selected_iwad {
                    GZDoomBuildSelection::Single => &v.iwads[0],
                    GZDoomBuildSelection::ListIndex(i) => &v.iwads[i],
                    GZDoomBuildSelection::FullPath(ref path) => path,
                }
                .clone(),
            ),
        }
    }
}

impl From<&dyn Storage> for Persistence {
    fn from(storage: &dyn Storage) -> Self {
        let gzdoom_build = storage.get_string("gzdoom_build");
        let primary_addon = storage.get_string("primary_addon");
        let secondary_addons = storage
            .get_string("secondary_addons")
            .map(|s| serde_yaml::from_str(s.as_str()).unwrap());
        let exargs = storage.get_string("exargs");
        let config = storage.get_string("config");
        let iwad = storage.get_string("iwad");
        Self {
            gzdoom_build,
            primary_addon,
            secondary_addons,
            exargs,
            config,
            iwad,
        }
    }
}

impl AddonManager {
    pub fn new(
        addons: AddonMap,
        app_options: AppOptions,
        config: Option<Persistence>,
    ) -> AddonManager {
        let mut primary_addons: Box<[String]> = iter::once(String::from("None"))
            .chain(
                addons
                    .iter()
                    .filter(|(_name, addon)| addon.secondary.is_none())
                    .map(|(name, _addon)| name.clone()),
            )
            .collect();
        primary_addons.sort_by(|a, b| {
            use std::cmp::Ordering::*;
            match (a.as_str(), b.as_str()) {
                ("None", _) => Less,
                (_, "None") => Greater,
                _ => a.cmp(b),
            }
        });
        let primary_addons = primary_addons;
        let mut secondary_addons: Box<[String]> = addons
            .iter()
            .filter(|(_name, addon)| addon.secondary.is_some())
            .map(|(name, _addon)| name.clone())
            .collect();
        secondary_addons.sort();
        let secondary_addons = secondary_addons;
        let selected_secondary_addons: Box<[bool]> =
            Box::from_iter(secondary_addons.iter().map(|_| true));
        let bpat = app_options.gzdoom_glob.unwrap_or(String::new());
        let builds: Box<[String]> = match glob::glob(&bpat) {
            Ok(paths) => paths
                .filter_map(Result::ok)
                .filter_map(|p| match is_executable(&p) {
                    true => p.to_str().map(str::to_string),
                    false => None,
                })
                .collect(),
            Err(_e) => Box::from([]),
        };
        let ipat = app_options.iwad_glob.unwrap_or(String::new());
        let iwads: Box<[String]> = match glob::glob(&ipat) {
            Ok(paths) => paths
                .filter_map(Result::ok)
                .filter_map(|p| match is_iwad(&p) {
                    true => p.to_str().map(str::to_string),
                    false => None,
                })
                .collect(),
            Err(_e) => Box::from([]),
        };

        // STEP: Load configuration
        let (
            selected_primary_addon,
            selected_secondary_addons,
            selected_gzdoom_build,
            selected_iwad,
            exargs,
            config,
        ) = config
            .as_ref()
            .map(|config| {
                let selected_primary_addon = primary_addons
                    .iter()
                    .position(|pa| {
                        config
                            .primary_addon
                            .as_ref()
                            .map(|s| s.eq(pa))
                            .unwrap_or(false)
                    })
                    .unwrap_or(0);
                let selected_secondary_addons = secondary_addons
                    .iter()
                    .map(|sa| {
                        config
                            .secondary_addons
                            .as_ref()
                            .map(|addons| addons.get(sa).copied().unwrap_or(true))
                            .unwrap_or(true)
                    })
                    .collect();
                let selected_gzdoom_build = match builds.len() {
                    0 => GZDoomBuildSelection::FullPath(
                        config.gzdoom_build.clone().unwrap_or_default(),
                    ),
                    1 => GZDoomBuildSelection::Single,
                    _ => GZDoomBuildSelection::ListIndex(
                        builds
                            .iter()
                            .position(|build| {
                                config
                                    .gzdoom_build
                                    .as_ref()
                                    .map(|gzd| gzd.as_str() == build)
                                    .unwrap_or(false)
                            })
                            .unwrap_or_default(),
                    ),
                };
                let selected_iwad = match iwads.len() {
                    0 => GZDoomBuildSelection::FullPath(config.iwad.clone().unwrap_or_default()),
                    1 => GZDoomBuildSelection::Single,
                    _ => GZDoomBuildSelection::ListIndex(
                        iwads
                            .iter()
                            .position(|iwad| {
                                config
                                    .iwad
                                    .as_ref()
                                    .map(|gzd| gzd.as_str() == iwad)
                                    .unwrap_or(false)
                            })
                            .unwrap_or_default(),
                    ),
                };
                let exargs = config.exargs.clone().unwrap_or_default();
                let config = config.config.clone().unwrap_or_default();
                (
                    selected_primary_addon,
                    selected_secondary_addons,
                    selected_gzdoom_build,
                    selected_iwad,
                    exargs,
                    config,
                )
            })
            .unwrap_or((
                0,
                selected_secondary_addons,
                GZDoomBuildSelection::default(),
                GZDoomBuildSelection::default(),
                Default::default(),
                Default::default(),
            ));
        AddonManager {
            builds,
            iwads,
            primary_addons,
            secondary_addons,
            addons,
            selected_primary_addon,
            selected_secondary_addons,
            selected_gzdoom_build,
            selected_iwad,
            quit_on_launch: app_options.quit_on_launch,
            exargs,
            config,
            ..Default::default()
        }
    }
    fn gzdoom_build(&self) -> &str {
        match &self.selected_gzdoom_build {
            GZDoomBuildSelection::Single => self
                .builds
                .get(0)
                .map(String::as_str)
                .expect("How did this happen?!"),
            GZDoomBuildSelection::ListIndex(index) => {
                self.builds.get(*index).map(String::as_str).unwrap_or("")
            }
            GZDoomBuildSelection::FullPath(path) => path.as_str(),
        }
    }
    fn iwad(&self) -> &str {
        match &self.selected_iwad {
            GZDoomBuildSelection::Single => self
                .iwads
                .get(0)
                .map(String::as_str)
                .expect("How did this happen?!"),
            GZDoomBuildSelection::ListIndex(index) => {
                self.iwads.get(*index).map(String::as_str).unwrap_or("")
            }
            GZDoomBuildSelection::FullPath(path) => path.as_str(),
        }
    }
    fn files_for_addon<'a>(&'a self, addon: Option<&'a AddonSpecification>) -> Vec<&'a String> {
        match addon {
            Some(addon) => {
                let mut files = vec![];
                for file in &addon.required {
                    files.push(file);
                }
                if let Some(optional) = &addon.optional {
                    for file in optional {
                        if File::open(file).is_ok() {
                            files.push(file);
                        }
                    }
                }
                files
            }
            None => vec![],
        }
    }
    fn primary_addon<'a>(&'a self) -> Vec<&'a String> {
        let name = self
            .primary_addons
            .get(self.selected_primary_addon)
            .map(String::as_str)
            .unwrap_or("");
        let addon = self.addons.get(name);
        self.files_for_addon(addon)
    }
    fn secondary_addons<'a>(&'a self) -> Vec<&'a String> {
        let addons: Vec<String> = self
            .secondary_addons
            .iter()
            .zip(self.selected_secondary_addons.iter())
            .filter_map(|(addon, &selected)| if selected { Some(addon) } else { None })
            .cloned()
            .collect();
        let mut addon_files = vec![];
        addons.iter().for_each(|addon| {
            let addon = self.addons.get(addon);
            addon_files.extend(self.files_for_addon(addon).into_iter());
        });
        addon_files
    }
    fn try_launch<'a>(&'a self) -> Result<(), LaunchError> {
        let gzdoom = self.gzdoom_build();
        let iwad = self.iwad();
        if File::open(&gzdoom).is_err() {
            return Err(LaunchError::GZDoomBuildNotOpenable);
        }
        if !is_executable(&gzdoom) {
            return Err(LaunchError::GZDoomBuildNotExecutable);
        }
        if File::open(&iwad).is_err() {
            return Err(LaunchError::IWADNotFound);
        }
        if !is_iwad(&iwad) {
            return Err(LaunchError::IWADNotIWAD);
        }
        let run_info = get_run_info(&self.exargs, &gzdoom);
        let primary_addon = self.primary_addon();
        let secondary_addons = self.secondary_addons();
        match Command::new(run_info.new_executable.unwrap_or(&gzdoom))
            .envs(env::vars())
            .envs(run_info.environment.iter().map(|(a, b)| (a, b.as_ref())))
            .args(run_info.arguments)
            .args(["-iwad", &iwad])
            .args(
                (if self.config.len() > 0 {
                    Some(["-config", &self.config])
                } else {
                    None
                })
                .iter()
                .flatten(),
            )
            .arg("-file")
            .args(primary_addon)
            .args(secondary_addons)
            .spawn()
        {
            Ok(mut child) => {
                if let Err(e) = child.wait() {
                    return Err(LaunchError::FailedWait(Box::from(e)));
                }
            }
            Err(e) => {
                return Err(LaunchError::LaunchFailed(Box::from(e)));
            }
        }
        Ok(())
    }
}

#[derive(Debug)]
enum LaunchError {
    GZDoomBuildNotOpenable,
    GZDoomBuildNotExecutable,
    IWADNotFound,
    IWADNotIWAD,
    LaunchFailed(Box<dyn Error>),
    FailedWait(Box<dyn Error>),
}

impl std::fmt::Display for LaunchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let thing_to_print = match self {
            LaunchError::GZDoomBuildNotOpenable => String::from("Cannot open GZDoom build"),
            LaunchError::GZDoomBuildNotExecutable => {
                String::from("Selected GZDoom build is not an executable!")
            }
            LaunchError::IWADNotFound => String::from("Cannot open IWAD"),
            LaunchError::IWADNotIWAD => String::from("Selected IWAD is not an IWAD!"),
            LaunchError::LaunchFailed(e) => format!("Could not launch GZDoom:\n{:?}", e),
            LaunchError::FailedWait(e) => format!("Failed to wait on child process:\n{:?}", e),
        };
        write!(f, "{}", thing_to_print)?;
        Ok(())
    }
}

impl Error for LaunchError {}

impl App for AddonManager {
    fn update(&mut self, ctx: &egui::Context, _eframe: &mut Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            match &mut self.selected_gzdoom_build {
                GZDoomBuildSelection::Single => {}
                GZDoomBuildSelection::ListIndex(bindex) => {
                    egui::ComboBox::from_label("GZDoom build")
                        .selected_text(self.builds.get(*bindex).unwrap_or(&String::from("None")))
                        .width(400.)
                        .show_ui(ui, |ui| {
                            self.builds.iter().enumerate().for_each(|(index, build)| {
                                ui.selectable_value(bindex, index, build);
                            });
                        });
                    ui.separator();
                }
                GZDoomBuildSelection::FullPath(path) => {
                    ui.horizontal(|ui| {
                        ui.label("GZDoom build:");
                        ui.add(egui::TextEdit::singleline(path));
                        if ui.button("Browse").clicked() {
                            if let Ok(choice) =
                                native_dialog::FileDialog::new().show_open_single_file()
                            {
                                if let Some(choice) = choice {
                                    if is_executable(&choice) {
                                        *path = String::from(choice.to_str().unwrap_or(""));
                                    } else {
                                        self.popup =
                                            Some(format!("{:?} is not executable!", choice));
                                    }
                                }
                            } else {
                                self.popup = Some(format!("File browser unavailable"));
                            }
                        }
                    });
                    ui.separator();
                }
            }

            match &mut self.selected_iwad {
                GZDoomBuildSelection::Single => {}
                GZDoomBuildSelection::ListIndex(bindex) => {
                    egui::ComboBox::from_label("IWAD")
                        .selected_text(self.builds.get(*bindex).unwrap_or(&String::from("None")))
                        .width(400.)
                        .show_ui(ui, |ui| {
                            self.builds.iter().enumerate().for_each(|(index, build)| {
                                ui.selectable_value(bindex, index, build);
                            });
                        });
                    ui.separator();
                }
                GZDoomBuildSelection::FullPath(path) => {
                    ui.horizontal(|ui| {
                        ui.label("IWAD:");
                        ui.add(egui::TextEdit::singleline(path));
                        if ui.button("Browse").clicked() {
                            if let Ok(choice) =
                                native_dialog::FileDialog::new().show_open_single_file()
                            {
                                if let Some(choice) = choice {
                                    if is_iwad(&choice) {
                                        *path = String::from(choice.to_str().unwrap_or(""));
                                    } else {
                                        self.popup = Some(format!("{:?} is not an IWAD!", choice));
                                    }
                                }
                            } else {
                                self.popup = Some(format!("File browser unavailable"));
                            }
                        }
                    });
                    ui.separator();
                }
            }

            egui::ComboBox::from_label("Primary addon")
                .selected_text(
                    self.primary_addons
                        .get(self.selected_primary_addon)
                        .unwrap_or(&String::from("None")),
                )
                .width(400.)
                .show_ui(ui, |ui| {
                    self.primary_addons
                        .iter()
                        .enumerate()
                        .for_each(|(index, addon)| {
                            ui.selectable_value(&mut self.selected_primary_addon, index, addon);
                        });
                });

            ui.separator();

            egui::CollapsingHeader::new("Secondary addons")
                .default_open(self.secondary_addons.len() <= 4)
                .show(ui, |ui| {
                    self.selected_secondary_addons
                        .iter_mut()
                        .zip(self.secondary_addons.iter())
                        .for_each(|(selected, name)| {
                            ui.checkbox(selected, name);
                        });
                });

            ui.separator();

            ui.horizontal(|ui| {
                ui.label("Extra arguments:");
                ui.text_edit_singleline(&mut self.exargs).on_hover_ui(|ui| {
                    ui.label("You can use %command% to set environment variables");
                    ui.label("and/or run GZDoom under another executable, just like");
                    ui.label("the Steam launch options. See this for more information:");
                    ui.hyperlink("https://superuser.com/q/954041");
                });
            });

            ui.horizontal(|ui| {
                ui.label("Configuration file name:");
                ui.text_edit_singleline(&mut self.config);
            });

            ui.separator();

            ui.horizontal(|ui| {
                if ui.button("Launch").clicked() {
                    if let Err(e) = self.try_launch() {
                        self.popup = Some(e.to_string());
                    }
                    if self.quit_on_launch {
                        self.quit = true;
                    }
                }

                if ui.button("Exit").clicked() {
                    self.quit = true;
                }
            });
        });
        if let Some(msg) = &self.popup {
            // Work around borrow checker. Argh.
            let mut open = true;
            let mut close = false;
            egui::Window::new("Message")
                .open(&mut open)
                .show(ctx, |ui| {
                    ui.label(msg);
                    if ui.button("OK").clicked() {
                        close = true;
                    }
                });
            if !open || close {
                self.popup = None;
            }
        }
    }
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        let data: Persistence = Persistence::from(&*self);
        data.save(storage);
    }
    fn persist_egui_memory(&self) -> bool {
        false
    }
    fn persist_native_window(&self) -> bool {
        false
    }
    /* fn on_exit(&mut self, _glc: Option<&Context>) {
        // Mutable to immutable reference
        let data = Persistence::from(&*self);
        let mut cfg_path =
            dirs::config_dir().unwrap_or(dirs::home_dir().unwrap_or(env::current_dir().unwrap()));
        cfg_path.push(&format!("Talon1024{0}Addon Manager{0}", DSEP));
        if let Err(e) = fs::create_dir_all(&cfg_path) {
            eprintln!("Could not save settings:\n{:?}", e);
            return;
        }
        cfg_path.push("addon_manager.yml");
        let yaml = serde_yaml::to_string(&data);
        match yaml {
            Ok(yaml) => {
                let file = OpenOptions::new()
                    .write(true)
                    .create(true)
                    .truncate(true)
                    .open(cfg_path);
                match file {
                    Ok(mut f) => {
                        if let Err(e) = f.write_all(&yaml.into_bytes()) {
                            eprintln!("Could not save settings:\n{:?}", e);
                        }
                    }
                    Err(e) => {
                        eprintln!("Could not save settings:\n{:?}", e);
                    }
                }
            }
            Err(e) => {
                eprintln!("Could not save settings:\n{:?}", e);
            }
        }
    } */
}
