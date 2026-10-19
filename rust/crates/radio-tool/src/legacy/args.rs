#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Flag,
    Help,
    Number,
    Text,
    List,
}

struct Spec {
    short: Option<char>,
    long: &'static str,
    kind: Kind,
}

const SPECS: &[Spec] = &[
    spec(Some('h'), "help", Kind::Help),
    spec(Some('l'), "list", Kind::Flag),
    spec(Some('d'), "device", Kind::Number),
    spec(None, "port", Kind::Text),
    spec(None, "ble", Kind::Text),
    spec(None, "ble-adapter", Kind::Text),
    spec(Some('i'), "in", Kind::Text),
    spec(Some('o'), "out", Kind::Text),
    spec(Some('f'), "flash", Kind::Flag),
    spec(Some('p'), "program", Kind::Flag),
    spec(None, "read-codeplug", Kind::Flag),
    spec(None, "info", Kind::Flag),
    spec(None, "get-status", Kind::Flag),
    spec(None, "get-time", Kind::Flag),
    spec(None, "set-time", Kind::Flag),
    spec(None, "dump-reg", Kind::Number),
    spec(None, "reboot", Kind::Flag),
    spec(None, "dump-bootloader", Kind::Flag),
    spec(None, "fw-info", Kind::Flag),
    spec(None, "wrap", Kind::Flag),
    spec(None, "unwrap", Kind::Flag),
    spec(None, "make-xor", Kind::Flag),
    spec(None, "codeplug-info", Kind::Flag),
    spec(None, "list-serial-models", Kind::Flag),
    spec(None, "list-ble", Kind::Flag),
    spec(Some('s'), "segment", Kind::List),
    spec(Some('r'), "radio", Kind::Text),
];

const fn spec(short: Option<char>, long: &'static str, kind: Kind) -> Spec {
    Spec { short, long, kind }
}
#[derive(Debug, Default)]
pub struct Parsed {
    seen: Vec<&'static str>,
    help: Option<String>,
    numbers: Vec<(&'static str, u16)>,
    texts: Vec<(&'static str, String)>,
    segments: Vec<String>,
}

impl Parsed {
    pub fn has(&self, long: &str) -> bool {
        self.seen.contains(&long)
    }
    pub fn is_empty(&self) -> bool {
        self.seen.is_empty()
    }
    pub fn help_section(&self) -> Option<&str> {
        self.help.as_deref()
    }
    pub fn text(&self, long: &str) -> Option<&str> {
        self.texts
            .iter()
            .rev()
            .find(|(k, _)| *k == long)
            .map(|(_, v)| v.as_str())
    }
    pub fn number(&self, long: &str) -> Option<u16> {
        self.numbers
            .iter()
            .rev()
            .find(|(k, _)| *k == long)
            .map(|(_, v)| *v)
    }
    pub fn segments(&self) -> &[String] {
        &self.segments
    }

    fn apply(&mut self, spec: &Spec, value: &str) -> Result<(), String> {
        match spec.kind {
            Kind::Flag => {
                parse_bool(value).ok_or_else(|| bad_value(value))?;
            }
            Kind::Help => self.help = Some(value.to_owned()),
            Kind::Number => {
                let n = parse_u16(value).ok_or_else(|| bad_value(value))?;
                self.numbers.push((spec.long, n));
            }
            Kind::Text => self.texts.push((spec.long, value.to_owned())),
            Kind::List => self.segments.extend(split_list(value)),
        }
        self.seen.push(spec.long);
        Ok(())
    }
}

fn implicit(kind: Kind) -> Option<&'static str> {
    match kind {
        Kind::Flag => Some("true"),
        Kind::Help => Some(""),
        _ => None,
    }
}

fn bad_value(value: &str) -> String {
    format!("Argument \u{2018}{value}\u{2019} failed to parse")
}

fn not_exists(name: &str) -> String {
    format!("Option \u{2018}{name}\u{2019} does not exist")
}

fn missing(name: &str) -> String {
    format!("Option \u{2018}{name}\u{2019} is missing an argument")
}
pub fn parse(args: &[String]) -> Result<Parsed, String> {
    let mut parsed = Parsed::default();
    let mut ix = 0;

    while let Some(arg) = args.get(ix) {
        if arg == "--" {
            break;
        }

        match classify(arg) {
            Shape::Long { name, value } => {
                let spec = SPECS
                    .iter()
                    .find(|s| s.long == name)
                    .ok_or_else(|| not_exists(name))?;
                match value {
                    Some(v) => parsed.apply(spec, v)?,
                    None => ix = take_value(&mut parsed, spec, name, args, ix)?,
                }
            }
            Shape::Group(group) => {
                let chars: Vec<char> = group.chars().collect();
                for (pos, c) in chars.iter().enumerate() {
                    let name = c.to_string();
                    let spec = SPECS
                        .iter()
                        .find(|s| s.short == Some(*c))
                        .ok_or_else(|| not_exists(&name))?;
                    if pos + 1 == chars.len() {
                        ix = take_value(&mut parsed, spec, &name, args, ix)?;
                    } else if let Some(v) = implicit(spec.kind) {
                        parsed.apply(spec, v)?;
                    } else {
                        let rest: String = chars.iter().skip(pos + 1).collect();
                        parsed.apply(spec, &rest)?;
                        break;
                    }
                }
            }
            Shape::BadSyntax => {
                return Err(format!(
                    "Argument \u{2018}{arg}\u{2019} starts with a - but has incorrect syntax"
                ));
            }
            Shape::Positional => {}
        }
        ix += 1;
    }

    Ok(parsed)
}

fn take_value(
    parsed: &mut Parsed,
    spec: &Spec,
    name: &str,
    args: &[String],
    ix: usize,
) -> Result<usize, String> {
    if let Some(v) = implicit(spec.kind) {
        parsed.apply(spec, v)?;
        return Ok(ix);
    }
    let value = args.get(ix + 1).ok_or_else(|| missing(name))?;
    parsed.apply(spec, value)?;
    Ok(ix + 1)
}

enum Shape<'a> {
    Long {
        name: &'a str,
        value: Option<&'a str>,
    },
    Group(&'a str),
    BadSyntax,
    Positional,
}

fn classify(arg: &str) -> Shape<'_> {
    let alnum = |c: char| c.is_ascii_alphanumeric();

    if let Some(body) = arg.strip_prefix("--") {
        let (name, value) = match body.split_once('=') {
            Some((n, v)) => (n, Some(v)),
            None => (body, None),
        };
        let mut chars = name.chars();
        let first_ok = chars.next().is_some_and(alnum);
        let rest_ok = name.len() >= 2 && chars.all(|c| alnum(c) || c == '-' || c == '_');
        let value_ok = value.is_none_or(|v| !v.contains(['\n', '\r']));
        if first_ok && rest_ok && value_ok {
            return Shape::Long { name, value };
        }
        return Shape::BadSyntax;
    }

    if let Some(body) = arg.strip_prefix('-') {
        if body.is_empty() {
            return Shape::Positional;
        }
        if body.chars().all(alnum) {
            return Shape::Group(body);
        }
        return Shape::BadSyntax;
    }

    Shape::Positional
}

fn parse_bool(text: &str) -> Option<bool> {
    match text {
        "t" | "T" | "true" | "True" | "1" => Some(true),
        "f" | "F" | "false" | "False" | "0" => Some(false),
        _ => None,
    }
}

fn parse_u16(text: &str) -> Option<u16> {
    if text.starts_with('-') {
        return None;
    }
    let (digits, base) = match text.strip_prefix("0x") {
        Some(hex) if !hex.is_empty() => (hex, 16),
        _ => (text, 10),
    };
    if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_alphanumeric()) {
        return None;
    }
    u16::from_str_radix(digits, base).ok()
}
fn split_list(text: &str) -> Vec<String> {
    if text.is_empty() {
        return vec![String::new()];
    }
    let mut parts: Vec<String> = text.split(',').map(str::to_owned).collect();
    if text.ends_with(',') {
        parts.pop();
    }
    parts
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    fn p(args: &[&str]) -> Result<Parsed, String> {
        parse(&args.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>())
    }

    #[test]
    fn short_options_group_and_take_the_rest() {
        let r = p(&["-ld", "3"]).unwrap();
        assert!(r.has("list"));
        assert_eq!(r.number("device"), Some(3));

        let r = p(&["-d0"]).unwrap();
        assert_eq!(r.number("device"), Some(0));

        assert_eq!(p(&["-lx"]).unwrap_err(), not_exists("x"));
        assert_eq!(p(&["-hwrap"]).unwrap_err(), not_exists("w"));
    }

    #[test]
    fn help_never_takes_the_next_argument() {
        let r = p(&["-h", "wrap"]).unwrap();
        assert_eq!(r.help_section(), Some(""));
        let r = p(&["--help=Wrap"]).unwrap();
        assert_eq!(r.help_section(), Some("Wrap"));
    }

    #[test]
    fn values_are_taken_even_when_they_look_like_options() {
        assert_eq!(p(&["-d", "-1"]).unwrap_err(), bad_value("-1"));
        let r = p(&["-i", "-o"]).unwrap();
        assert_eq!(r.text("in"), Some("-o"));
        assert_eq!(p(&["-d"]).unwrap_err(), missing("d"));
        assert_eq!(p(&["--dump-reg"]).unwrap_err(), missing("dump-reg"));
    }

    #[test]
    fn numbers_take_decimal_or_hex_and_the_last_wins() {
        assert_eq!(p(&["-d", "0x1"]).unwrap().number("device"), Some(1));
        assert_eq!(
            p(&["-d", "1", "-d", "2"]).unwrap().number("device"),
            Some(2)
        );
        assert_eq!(p(&["--device=0x2"]).unwrap().number("device"), Some(2));
        for bad in ["abc", "70000", "1.5", "0x", "0X1", ""] {
            assert_eq!(p(&["-d", bad]).unwrap_err(), bad_value(bad), "{bad}");
        }
    }

    #[test]
    fn a_flag_counts_whatever_its_value() {
        assert!(p(&["--list=false"]).unwrap().has("list"));
        assert!(p(&["--list=true"]).unwrap().has("list"));
        assert_eq!(p(&["--list=x"]).unwrap_err(), bad_value("x"));
    }

    #[test]
    fn bad_syntax_and_unknown_options() {
        assert!(p(&["-d=1"]).unwrap_err().contains("incorrect syntax"));
        assert!(p(&["--x"]).unwrap_err().contains("incorrect syntax"));
        assert_eq!(p(&["--bogus"]).unwrap_err(), not_exists("bogus"));
        assert_eq!(p(&["--In", "x"]).unwrap_err(), not_exists("In"));
    }

    #[test]
    fn stray_words_and_double_dash_leave_nothing_set() {
        assert!(p(&["foo"]).unwrap().is_empty());
        assert!(p(&["-"]).unwrap().is_empty());
        assert!(p(&["--", "-l"]).unwrap().is_empty());
        assert!(p(&["foo", "-l"]).unwrap().has("list"));
    }

    #[test]
    fn segments_accumulate_and_split_on_commas() {
        let r = p(&["-s", "0:a,0x10:b", "--segment", "4:c"]).unwrap();
        assert_eq!(r.segments(), ["0:a", "0x10:b", "4:c"]);
        assert_eq!(split_list("a,b,"), ["a", "b"]);
        assert_eq!(split_list("a,,b"), ["a", "", "b"]);
    }
}
