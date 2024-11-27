use anyhow::{anyhow, bail, Result};

#[derive(Debug, Eq, PartialEq)]
enum State {
    None,
    OpeningBrace,
    ClosingBrace,
}

pub fn format(fmt: &str, args: &[&str]) -> Result<String> {
    let mut result = String::new();
    let mut state = State::None;
    let mut arg_iter = args.iter();

    for c in fmt.chars() {
        match state {
            State::None => match c {
                '{' => state = State::OpeningBrace,
                '}' => state = State::ClosingBrace,
                _ => result.push(c),
            },
            State::OpeningBrace => match c {
                '{' => {
                    state = State::None;
                    result.push('{');
                }
                '}' => {
                    state = State::None;
                    result.push_str(
                        arg_iter
                            .next()
                            .ok_or_else(|| anyhow!("unmatched number of arguments."))?,
                    );
                }
                _ => bail!("unmatched brace."),
            },
            State::ClosingBrace => match c {
                '}' => {
                    state = State::None;
                    result.push('}');
                }
                _ => bail!("unmatched brace."),
            },
        }
    }

    if state != State::None {
        bail!("unmatched brace.");
    }

    if arg_iter.next().is_some() {
        bail!("unmatched number of arguments.");
    }

    Ok(result)
}
