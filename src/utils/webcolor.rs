use anyhow::{anyhow, bail, Result};

pub fn parse_webcolor(expr: &str) -> Result<u32> {
    let hex = expr
        .strip_prefix('#')
        .ok_or_else(|| anyhow!("invalid webcolor format."))?;

    if hex.len() != 6 {
        bail!("invalid webcolor format.");
    }

    Ok(u32::from_str_radix(hex, 16)?)
}
