use std::{
    collections::HashMap,
    error::Error,
    env,
    fs::{self, File},
    io::Read,
    path::Path,
    iter,
    process,
    rc::Rc,
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

#[derive(Debug, Clone, Default)]
struct AppOptions {
    quit_on_launch: bool,
    gzdoom_glob: Option<String>,
}

fn main() -> Result<(), Box<dyn Error>> {
    /*
    let options = eframe::NativeOptions {
        initial_window_size: Some(Vec2 {x: 550., y: 300.}),
        vsync: false,
        ..Default::default()
    };
    */
    use glutin::{Api, GlRequest, dpi::{Size, LogicalSize}};
    use egui_glow::{painter::Context, EguiGlow};
    let el = glutin::event_loop::EventLoop::new();
    let wb = glutin::window::WindowBuilder::new()
        .with_title("Doom 64 CE launcher")
        .with_inner_size(Size::new(LogicalSize::new(550.0f32, 300.0f32)));
    let cb = glutin::ContextBuilder::new()
        .with_gl(GlRequest::Specific(Api::OpenGl, (3, 3)))
        .build_windowed(wb, &el)?;
    let cb = unsafe { cb.make_current() }.map_err(|(_old_ctx, err)| err)?;
    let ctx = Rc::from(unsafe {
        Context::from_loader_function(|name| cb.get_proc_address(name)) });
    let mut eguiglow = EguiGlow::new(cb.window(), ctx.clone());
    let app_options = AppOptions {
        quit_on_launch: env::args().any(|arg| arg == "--quit-on-launch"),
        gzdoom_glob: env::args().skip_while(|arg| arg != "--gzdoom-glob").skip(1).next(),
        ..Default::default()
    };
    let addons = get_addons(None);
    let mut app: Box<dyn App> = match addons {
        Ok(addons) => {
            Box::new(AddonManager::new(addons, app_options))
            /*
            eframe::run_native("Doom 64 CE launcher", options,
                Box::new(move |_| Box::new(AddonManager::new(addons, app_options)))); */
        },
        Err(error) => {
            let message = format!("{:#?}", error);
            Box::new(ErrorMessage::from(message))
            /*
            eframe::run_native("Doom 64 CE launcher", options,
                Box::new(|_| Box::new(ErrorMessage(message)))); */
        }
    };
    el.run(move |event, _, control_flow| {
        // Some code copied from https://github.com/emilk/egui/blob/master/egui_glow/examples/pure_glow.rs
        let mut redraw = || {
            use glutin::event_loop::ControlFlow;
            let needs_repaint = eguiglow.run(cb.window(), |ctx| app.update(ctx));
            let quit = app.quit();

            *control_flow = if quit {
                ControlFlow::Exit
            } else if needs_repaint {
                cb.window().request_redraw();
                ControlFlow::Poll
            } else {
                ControlFlow::Wait
            };

            eguiglow.paint(cb.window());
            cb.swap_buffers().unwrap();
        };

        // Copied from https://github.com/emilk/egui/blob/master/egui_glow/examples/pure_glow.rs
        match event {
            // Platform-dependent event handlers to workaround a winit bug
            // See: https://github.com/rust-windowing/winit/issues/987
            // See: https://github.com/rust-windowing/winit/issues/1619
            glutin::event::Event::RedrawEventsCleared if cfg!(windows) => redraw(),
            glutin::event::Event::RedrawRequested(_) if !cfg!(windows) => redraw(),

            glutin::event::Event::WindowEvent { event, .. } => {
                use glutin::event::WindowEvent;
                if matches!(event, WindowEvent::CloseRequested | WindowEvent::Destroyed) {
                    *control_flow = glutin::event_loop::ControlFlow::Exit;
                }

                if let glutin::event::WindowEvent::Resized(physical_size) = &event {
                    cb.resize(*physical_size);
                } else if let glutin::event::WindowEvent::ScaleFactorChanged {
                    new_inner_size,
                    ..
                } = &event
                {
                    cb.resize(**new_inner_size);
                }

                eguiglow.on_event(&event);

                cb.window().request_redraw(); // TODO: ask egui if the events warrants a repaint instead
            }
            glutin::event::Event::LoopDestroyed => {
                eguiglow.destroy();
            }

            _ => (),
        }
    });
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
enum GZDoomBuildSelection {
    ListIndex(usize),
    FullPath(String),
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
                0 => GZDoomBuildSelection::FullPath(String::new()),
                _ => GZDoomBuildSelection::ListIndex(0),
            },
            quit_on_launch: app_options.quit_on_launch,
            ..Default::default()
        }
    }
    fn gzdoom_build(&self) -> &str {
        match &self.selected_gzdoom_build {
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

trait App {
    fn update(&mut self, ctx: &egui::Context);
    fn quit(&self) -> bool;
}

impl App for AddonManager {
    fn update(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            match &mut self.selected_gzdoom_build {
                GZDoomBuildSelection::ListIndex(bindex) => {
                    egui::ComboBox::from_label("GZDoom build")
                    .selected_text(self.builds.get(*bindex)
                        .unwrap_or(&String::from("None")))
                    .width(400.).show_ui(ui, |ui| {
                        self.builds.iter().enumerate().for_each(|(index, build)| {
                            ui.selectable_value(bindex, index, build);
                        });
                    });
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
                }
            }

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
            if ui.button("Exit").clicked() {
                self.1 = true;
            }
        });
    }
    fn quit(&self) -> bool {
        self.1
    }
}
