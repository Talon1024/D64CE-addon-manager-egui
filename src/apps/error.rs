use crate::ephraim::App;

pub struct ErrorMessage(String, bool);
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
