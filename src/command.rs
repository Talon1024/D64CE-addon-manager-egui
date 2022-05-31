use crate::cmdlineparse;
use std::borrow::Cow;

#[derive(Debug, Clone, Default)]
pub struct RunInfo<'a> {
	pub environment: Vec<(&'a str, Cow<'a, str>)>,
	pub new_executable: Option<&'a str>,
	pub arguments: Vec<&'a str>
}

pub fn get_run_info<'a>(args: &'a str, orig_exe: &'a str) -> RunInfo<'a> {
	let command: Option<(&str, &str)> = args.split_once("%command%");
	match command {
		Some((prefix, suffix)) => {
			let pre_args = cmdlineparse::parse_cmdline(prefix);
			let mut run_info = RunInfo::default();
			let mut parsing_env = true;
			pre_args.for_each(|arg| {
				if parsing_env {
					let pair: Option<(&str, &str)> = arg.split_once('=');
					match pair {
						Some((key, val)) => {
							run_info.environment.push(
								(key, cmdlineparse::dequote(val))
							);
						},
						None => {
							parsing_env = false;
						}
					}
				}
				// These two "ifs" are separate so that arguments can be
				// parsed after parsing_env is set to false
				if !parsing_env {
					match run_info.new_executable {
						Some(_) => { run_info.arguments.push(arg); },
						None => { run_info.new_executable.get_or_insert(arg); }
					}
				}
			});
			if run_info.new_executable.is_some() {
				run_info.arguments.push(orig_exe);
			}
			cmdlineparse::parse_cmdline(suffix).for_each(|arg| {
				run_info.arguments.push(arg.trim_matches('"'));
			});
			run_info
		},
		None => {
			RunInfo {
				arguments: cmdlineparse::parse_cmdline(args).collect(),
				..Default::default()
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn no_command() {
		let arghs = "CUP=TEA FOOL=BARF mangohud booba.wad feet.wad";
		let actual = get_run_info(arghs, "gzdoom");
		let expected_args = vec!["CUP=TEA", "FOOL=BARF", "mangohud", "booba.wad", "feet.wad"];
		assert_eq!(actual.arguments, expected_args);
	}

	#[test]
	fn yes_command() {
		let arghs = "CUP=TEA FOOL=BARF mangohud %command% booba.wad feet.wad";
		let actual = get_run_info(arghs, "gzdoom");
		let expected_env: Vec<(&str, Cow<str>)> = vec![("CUP", Cow::from("TEA")), ("FOOL", Cow::from("BARF"))];
		let expected_exe = Some("mangohud");
		let expected_args = vec!["gzdoom", "booba.wad", "feet.wad"];

		assert_eq!(actual.arguments, expected_args);
		assert_eq!(actual.new_executable, expected_exe);
		actual.environment.iter().zip(expected_env.iter()).for_each(|(key, val)| {
			assert_eq!(key, val);
		});
	}

	#[test]
	fn no_suffix() {
		let arghs = "ENABLE_VKBASALT=1 mangohud %command%";
		let actual = get_run_info(arghs, "gzdoom");
		let expected_env: Vec<(&str, Cow<str>)> = vec![("ENABLE_VKBASALT", Cow::from("1"))];
		let expected_exe = Some("mangohud");
		let expected_args = vec!["gzdoom"];

		assert_eq!(actual.arguments, expected_args);
		assert_eq!(actual.new_executable, expected_exe);
		actual.environment.iter().zip(expected_env.iter()).for_each(|(key, val)| {
			assert_eq!(key, val);
		});
	}

	#[test]
	fn with_spaces_and_quotes() {
		let arghs = "A=\"\\\"Quotes\\\" and spaces\\\\, oh my!\" BOY=good %command% -glversion 4.2";
		let actual = get_run_info(arghs, "gzdoom");
		let expected_env: Vec<(&str, Cow<str>)> = vec![("A", Cow::from("\"Quotes\" and spaces\\, oh my!")), ("BOY", Cow::from("good"))];
		let expected_exe = None;
		let expected_args = vec!["-glversion", "4.2"];

		assert_eq!(actual.arguments, expected_args);
		assert_eq!(actual.new_executable, expected_exe);
		actual.environment.iter().zip(expected_env.iter()).for_each(|(key, val)| {
			assert_eq!(key, val);
		});
	}
}
