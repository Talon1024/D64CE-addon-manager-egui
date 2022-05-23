use std::{
    error::Error,
    env,
    fs::File,
    iter,
};

mod addon;
mod command;
mod ephraim;
mod exe;

use addon::{AddonMap, AddonSpecification};
use ephraim::{App, AppWindow};
use exe::is_executable;

#[derive(Debug, Clone, Default)]
struct AppOptions {
    quit_on_launch: bool,
    gzdoom_glob: Option<String>,
}

fn main() -> Result<(), Box<dyn Error>> {
    let app_options = AppOptions {
        quit_on_launch: env::args().any(|arg| arg == "--quit-on-launch"),
        gzdoom_glob: env::args().skip_while(|arg| arg != "--gzdoom-glob").skip(1).next(),
        ..Default::default()
    };
    let addons = addon::get_addons(None);
    let app: Box<dyn App> = match addons {
        Ok(addons) => {
            Box::new(AddonManager::new(addons, app_options))
        },
        Err(error) => {
            let message = format!("{:#?}", error);
            Box::new(ErrorMessage::from(message))
        }
    };
    let win = AppWindow::new(app)?;
    win.run();
}

#[derive(Debug, Clone)]
enum GZDoomBuildSelection {
    Single, // Hide GZDoom build selector
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
    addons: AddonMap,
    primary_addons: Box<[String]>,
    secondary_addons: Box<[String]>,
    selected_primary_addon: usize,
    selected_secondary_addons: Box<[bool]>,
    selected_gzdoom_build: GZDoomBuildSelection,
    quit_on_launch: bool,
    popup: Option<String>,
    quit: bool,
    exargs: String,
    config: String,
}

impl AddonManager {
    pub fn new(addons: AddonMap, app_options: AppOptions) -> AddonManager {
        let primary_addons: Box<[String]> = iter::once(String::from("None")).chain(addons.iter().filter(|(_name, addon)| {
            addon.secondary.is_none()
        }).map(|(name, _addon)| name.clone())).collect();
        let secondary_addons: Box<[String]> = addons.iter().filter(|(_name, addon)| {
            addon.secondary.is_some()
        }).map(|(name, _addon)| name.clone()).collect();
        let selected_secondary_addons: Box<[bool]> = Box::from_iter(secondary_addons.iter().map(|_| true));
        let pat = app_options.gzdoom_glob.unwrap_or(String::from(""));
        let builds: Box<[String]> = match glob::glob(&pat) {
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
        let build_count = builds.len();
        AddonManager {
            builds,
            primary_addons,
            secondary_addons,
            addons,
            selected_primary_addon: 0,
            selected_secondary_addons,
            selected_gzdoom_build: match build_count {
                1 => GZDoomBuildSelection::Single,
                0 => GZDoomBuildSelection::FullPath(String::new()),
                _ => GZDoomBuildSelection::ListIndex(0),
            },
            quit_on_launch: app_options.quit_on_launch,
            ..Default::default()
        }
    }
    fn gzdoom_build(&self) -> &str {
        match &self.selected_gzdoom_build {
            GZDoomBuildSelection::Single => self.builds.get(0).map(String::as_str).expect("How did this happen?!"),
            GZDoomBuildSelection::ListIndex(index) => self.builds.get(*index).map(String::as_str).unwrap_or(""),
            GZDoomBuildSelection::FullPath(path) => path.as_str()
        }
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

impl App for AddonManager {
    fn update(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            match &mut self.selected_gzdoom_build {
                GZDoomBuildSelection::Single => {},
                GZDoomBuildSelection::ListIndex(bindex) => {
                    egui::ComboBox::from_label("GZDoom build")
                    .selected_text(self.builds.get(*bindex)
                        .unwrap_or(&String::from("None")))
                    .width(400.).show_ui(ui, |ui| {
                        self.builds.iter().enumerate().for_each(|(index, build)| {
                            ui.selectable_value(bindex, index, build);
                        });
                    });
                    ui.separator();
                },
                GZDoomBuildSelection::FullPath(path) => {
                    ui.horizontal(|ui| {
                        ui.label("GZDoom build:");
                        ui.add(egui::TextEdit::singleline(path));
                        if ui.button("Browse").clicked() {
                            if let Ok(choice) = native_dialog::FileDialog::new()
                            .show_open_single_file() {
                                if let Some(choice) = choice {
                                    if is_executable(&choice) {
                                        *path = String::from(choice.to_str().unwrap_or(""));
                                    } else {
                                        self.popup = Some(format!("{:?} is not executable!", choice));
                                    }
                                }
                            }
                        }
                    });
                    ui.separator();
                }
            }

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
                ui.label("Extra arguments:");
                ui.text_edit_singleline(&mut self.exargs).on_hover_ui(|ui| {
                    ui.label("You can use %command% to set environment variables");
                    ui.label("and/or run GZDoom under another executable, just like");
                    ui.label("the Steam launch options. See this for more information:");
                    ui.hyperlink("https://superuser.com/questions/954041");
                });
            });

            ui.horizontal(|ui| {
                ui.label("Configuration file name:");
                ui.text_edit_singleline(&mut self.config);
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
            .open(&mut open).show(ctx, |ui| {
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
    fn quit(&self) -> bool {
        self.quit
    }
}

struct ErrorMessage(String, bool);
impl From<String> for ErrorMessage {
    fn from(s: String) -> Self {
        ErrorMessage(s, false)
    }
}
impl App for ErrorMessage {
    fn update(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Error!");
            ui.label(&self.0);
            ui.separator();
            ui.label("This program is a helper for Doom mod launcher scripts.");
            ui.label("Users may select one primary addon, and any secondary addons.");
            ui.horizontal(|ui| {
                ui.label("This program reads addon information from");
                ui.code("addons.yml");
                ui.label(". This file should");
            });
            ui.label("be in the directory you launched this program from.");
            ui.label("Supported command line arguments:");
            egui::Grid::new("command_line_arguments").show(ui, |ui| {
                ui.code("--gzdoom-glob ptn");
                ui.vertical(|ui| {
                    ui.label("A 'glob' pattern for finding GZDoom executables.");
                    ui.horizontal(|ui| {
                        ui.label("See the");
                        ui.hyperlink_to("glob", "https://docs.rs/glob/0.3.0/glob/");
                        ui.label("crate documentation for more info");
                    });
                });
                ui.end_row();
                ui.code("--quit-on-launch");
                ui.label("Quit this program when you launch the game.");
                ui.end_row();
            });
            if ui.button("Exit").clicked() {
                self.1 = true;
            }
        });
    }
    fn quit(&self) -> bool {
        self.1
    }
}
