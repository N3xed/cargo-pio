use std::iter::FromIterator;

use super::{Arg, ArgDef};

pub struct Args(Vec<String>);

impl FromIterator<String> for Args {
    fn from_iter<T: IntoIterator<Item = String>>(iter: T) -> Self {
        Self(iter.into_iter().collect::<Vec<_>>())
    }
}

impl Args {
    pub fn parse<const N: usize>(
        &mut self,
        defs: [&ArgDef<'_, '_>; N],
    ) -> [Option<Option<String>>; N] {
        defs.parse_from(&mut self.0)
    }
}

impl super::ArgDef<'_, '_> {
    fn is_name_eq(&self, s: &str) -> bool {
        self.name == s || self.alias.iter().any(|a| *a == s)
    }

    fn parse_value(&self, i: usize, args: &mut Vec<String>) -> Option<Option<String>> {
        let arg = &args[i];

        let hyphen_count = arg.chars().take_while(|s| *s == '-').count();
        if self.opts.is_hyphen_count(hyphen_count) {
            return None;
        }

        let arg = &arg[hyphen_count..];
        match self.arg {
            Arg::Flag => {
                if self.is_name_eq(arg) {
                    args.remove(i);
                    Some(None)
                } else {
                    None
                }
            }
            Arg::Option => {
                let mut sep_len = None;
                let name_len = if arg.starts_with(self.name)
                    && self
                        .opts
                        .parse_value_sep(&arg[self.name.len()..], &mut sep_len)
                {
                    Some(self.name.len())
                } else {
                    // Do the same as above for every alias.
                    self.alias
                        .iter()
                        .find(|&&s| {
                            arg.starts_with(s)
                                && self.opts.parse_value_sep(&arg[s.len()..], &mut sep_len)
                        })
                        .map(|s| s.len())
                };

                if let Some(name_len) = name_len {
                    if let Some(sep_len) = sep_len {
                        Some(Some(args.remove(i).split_off(name_len + sep_len)))
                    } else {
                        let end_index = (i + 1).min(args.len() - 1);
                        Some(args.drain(i..=end_index).nth(1))
                    }
                } else {
                    None
                }
            }
        }
    }
}

pub trait ParseFrom<const N: usize> {
    fn parse_from(&self, args: &mut Vec<String>) -> [Option<Option<String>>; N];
}

impl<'a, 'b, const N: usize> ParseFrom<N> for [&ArgDef<'a, 'b>; N] {
    fn parse_from(&self, args: &mut Vec<String>) -> [Option<Option<String>>; N] {
        const INIT: Option<Option<String>> = None;
        let mut results = [INIT; N];

        let mut i = 0;
        while i < args.len() {
            let mut removed = false;
            for (def_i, def) in self.iter().enumerate() {
                let result = def.parse_value(i, args);
                removed = result.is_some();
                results[def_i] = result;
            }

            if !removed {
                i += 1;
            }
        }

        results
    }
}
