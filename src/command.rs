#[derive(Debug, Clone, Default)]
pub struct RunInfo {
	pub environment: Vec<(String, String)>,
	pub new_executable: Option<String>,
	pub arguments: Vec<String>
}

pub fn get_run_info(args: &str) -> RunInfo {
	let command: Option<(&str, &str)> = args.split_once("%command%");
	match command {
		Some((prefix, suffix)) => {
			let pre_args = prefix.split_ascii_whitespace();
			let mut run_info = RunInfo::default();
			let mut parsing_env = true;
			pre_args.for_each(|arg| {
				if parsing_env {
					let pair: Option<(&str, &str)> = arg.split_once('=');
					match pair {
						Some((key, val)) => {
							run_info.environment.push(
								(key.to_string(), val.to_string()));
						},
						None => {
							parsing_env = false;
						}
					}
				}
				// These two "ifs" are separate so that arguments can be
				// parsed after parsing_env is set to false
				if !parsing_env {
					let argstr = arg.to_string();
					match run_info.new_executable {
						Some(_) => { run_info.arguments.push(argstr); },
						None => { run_info.new_executable.get_or_insert(argstr); }
					}
				}
			});
			suffix.split_ascii_whitespace().for_each(|arg| {
				run_info.arguments.push(arg.to_string());
			});
			run_info
		},
		None => {
			RunInfo {
				arguments: args.split_ascii_whitespace().map(str::to_string).collect(),
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
		let actual = get_run_info(arghs);
		let expected_args: Vec<String> = ["CUP=TEA", "FOOL=BARF", "mangohud", "booba.wad", "feet.wad"].map(str::to_string).to_vec();
		assert_eq!(actual.arguments, expected_args);
	}

	#[test]
	fn yes_command() {
		let arghs = "CUP=TEA FOOL=BARF mangohud %command% booba.wad feet.wad";
		let actual = get_run_info(arghs);
		let expected_env = ["CUP=TEA", "FOOL=BARF"].map(str::to_string).to_vec();
		let expected_exe = Some(String::from("mangohud")); 
		let expected_args = ["booba.wad", "feet.wad"].map(str::to_string).to_vec();
		assert_eq!(actual.arguments, expected_args);
		assert_eq!(actual.new_executable, expected_exe);
		actual.environment.iter().for_each(|(key, val)| {
			let pairstr = format!("{}={}", key, val);
			assert!(expected_env.contains(&pairstr));
		});
	}
}
