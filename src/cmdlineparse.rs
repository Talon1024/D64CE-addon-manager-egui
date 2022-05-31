use std::borrow::Cow;

#[derive(Debug, Clone, Default)]
pub struct CommandLineParser<'a> {
	text: &'a str,
	pos: usize,
	escape: bool,
	in_quotes: bool,
}

impl<'a> Iterator for CommandLineParser<'a> {
	type Item = &'a str;

	fn next(&mut self) -> Option<Self::Item> {
		let start = self.pos;
		let start = start + self.text.bytes().skip(start).position(|ch| {
			!ch.is_ascii_whitespace()
		})?;
		let end = self.text.bytes().skip(start).position(|ch: u8| {
			if ch == b'\\' && !self.escape {
				self.escape = true;
			} else if ch == b'"' {
				self.in_quotes = !self.in_quotes;
			} else {
				self.escape = false;
			}
			!self.escape && !self.in_quotes && ch.is_ascii_whitespace()
		}).unwrap_or(self.text.len().saturating_sub(start)) + start;
		self.pos = end;
		Some(&self.text[start..end])
	}
}

pub fn parse_cmdline<'a>(text: &'a str) -> CommandLineParser<'a> {
	CommandLineParser {
		text,
		..Default::default()
	}
}

pub fn dequote<'a>(text: &'a str) -> Cow<'a, str> {
	if text.starts_with('"') && text.ends_with('"') {
		let text = Cow::from(text.trim_matches('"'));
		if text.contains('\\') {
			let mut eschar = false;
			Cow::from(String::from_utf8(text.bytes().filter(|ch| {
				if !eschar && ch == &b'\\' { eschar = true; }
				else if eschar { eschar = false; }
				!eschar
			}).collect()).unwrap())
		} else {
			text
		}
	} else {
		Cow::from(text)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn mixed() {
		let cmdline = "A=\"Has spaces\" B=nospaces Cnoeq D=\"escaped \\\"quotation\\\" marks\" E F";
		let expected = ["A=\"Has spaces\"", "B=nospaces", "Cnoeq", "D=\"escaped \\\"quotation\\\" marks\"", "E", "F"];
		let parser = parse_cmdline(cmdline);

		parser.zip(expected.into_iter()).for_each(|(actual, expected)| {
			assert_eq!(actual, expected);
		});
	}

	#[test]
	fn dequoted() {
		let cmdline = "A=\"Has spaces\" B=nospaces Cnoeq D=\"escaped \\\"quotation\\\" marks\" E F";
		let expected = [
			Some(("A", Cow::from("Has spaces"))),
			Some(("B", Cow::from("nospaces"))),
			None,
			Some(("D", Cow::from("escaped \"quotation\" marks"))),
			None, None];
		let parser = parse_cmdline(cmdline).map(|th| {
			let g = th.split_once('=')?;
			Some((g.0, dequote(g.1)))
		});

		parser.zip(expected.into_iter()).for_each(|(actual, expected)| {
			assert_eq!(actual, expected);
		});
	}
}
