use std::fmt::Display;

use bitflags::bitflags;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Arg {
    /// A flag with a name (ex. `-n` or `--name`).
    Flag,
    /// An option with a `name` and `value`
    ///
    /// Could be parsed as:
    /// - `--name=value`
    /// - `--name value`
    /// - `--namevalue`
    /// - `-nvalue`
    /// - `-namevalue`
    /// - `-name value`
    /// - `-name=value`
    ///
    /// Is serialized as (default):
    /// - `-nvalue` if `name` is a single character,
    /// - `--name=value` otherwise.
    Option,
}

impl Arg {
    /// Create an [`ArgDef`] from a `Arg::Flag` with `name`.
    pub const fn flag(name: &str) -> ArgDef<'_, 'static> {
        Self::Flag.with_name(name)
    }

    /// Create a [`ArgDef`] from an `Arg::Option` with `name`.
    pub const fn option(name: &str) -> ArgDef<'_, 'static> {
        Self::Option.with_name(name)
    }

    /// Create a [`ArgDef`] from this `Arg` with `name`.
    pub const fn with_name<'a>(self, name: &'a str) -> ArgDef<'a, 'static> {
        ArgDef {
            arg: self,
            name,
            alias: &[],
            opts: ArgOpts::empty(),
        }
    }
}

bitflags! {
    pub struct ArgOpts: u32 {
        const SINGLE_HYPHEN = (1 << 0);
        const DOUBLE_HYPHEN = (1 << 1);
        const VALUE_SEP_NO_SPACE = (1 << 2);
        const VALUE_SEP_EQUALS = (1 << 3);
        const VALUE_SEP_NEXT_ARG = (1 << 4);
    }
}

impl ArgOpts {
    pub const fn is_hyphen_count(self, count: usize) -> bool {
        (count == 1 && self.contains(Self::SINGLE_HYPHEN))
            || (count == 2 && self.contains(Self::DOUBLE_HYPHEN))
    }

    pub(super) fn parse_value_sep(self, s: &str, out_sep_len: &mut Option<usize>) -> bool {
        let c = s.chars().nth(0);
        let (result, sep_len) = match c {
            Some('=') if self.contains(Self::VALUE_SEP_EQUALS) => (true, Some(1)),
            None if self.contains(Self::VALUE_SEP_NEXT_ARG) => (true, None),
            c if self.contains(Self::VALUE_SEP_NO_SPACE) => (true, Some(0)),
            _ => (false, None),
        };
        *out_sep_len = sep_len;
        result
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ArgDef<'s, 'a> {
    pub arg: Arg,
    pub name: &'s str,
    pub alias: &'a [&'a str],
    pub opts: ArgOpts,
}

impl<'s, 'a> ArgDef<'s, 'a> {
    /// Set the `alias`(s) for this definition.
    pub const fn with_alias<'b>(self, alias: &'b [&'b str]) -> ArgDef<'s, 'b> {
        ArgDef {
            alias,
            arg: self.arg,
            name: self.name,
            opts: self.opts,
        }
    }

    /// Set the options for this definition.
    pub const fn with_opts(mut self, opts: ArgOpts) -> ArgDef<'s, 'a> {
        self.opts = opts;
        self
    }

    /// Generate individual arguments from this argument definition and a `value`.
    ///
    /// The `value` is ignored if this definition is a [`Arg::Flag`].
    pub fn format(&self, value: Option<&str>) -> FormattedArg {
        let ArgDef {
            arg, name, opts, ..
        } = *self;

        match arg {
            Arg::Flag if opts.is_empty() => {
                let second_hyphen = if self.name.len() > 1 { "-" } else { "" };

                FormattedArg::One(format!("-{}{}", second_hyphen, self.name))
            }
            Arg::Flag => {
                let second_hyphen = if opts.contains(ArgOpts::SINGLE_HYPHEN) {
                    ""
                } else {
                    "-"
                };

                FormattedArg::One(format!("-{}{}", second_hyphen, self.name))
            }
            Arg::Option => {
                let sep = if opts.contains(ArgOpts::VALUE_SEP_EQUALS) {
                    Some("=")
                } else if opts.contains(ArgOpts::VALUE_SEP_NO_SPACE) {
                    Some("")
                } else {
                    None
                };

                let second_hyphen = if opts.contains(ArgOpts::SINGLE_HYPHEN) {
                    ""
                } else if opts.contains(ArgOpts::DOUBLE_HYPHEN) {
                    "-"
                } else {
                    if name.len() > 1 {
                        "-"
                    } else {
                        ""
                    }
                };

                if let Some(sep) = sep {
                    let f = format!("-{}{}{}{}", second_hyphen, name, sep, value.unwrap());
                    FormattedArg::One(f)
                } else {
                    let f = format!("-{}{}", second_hyphen, name);
                    FormattedArg::Two(f, value.unwrap().into())
                }
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FormattedArg {
    None,
    One(String),
    Two(String, String),
}

impl Iterator for FormattedArg {
    type Item = String;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Two(first, second) => {
                let first = std::mem::replace(first, String::new());
                let second = std::mem::replace(second, String::new());
                *self = Self::One(second);
                Some(first)
            }
            Self::One(first) => {
                let first = std::mem::replace(first, String::new());
                *self = Self::None;
                Some(first)
            }
            _ => None,
        }
    }
}

impl Display for FormattedArg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Two(first, second) => write!(f, "{}{}", first, second),
            Self::One(first) => write!(f, "{}", first),
            Self::None => Ok(()),
        }
    }
}
