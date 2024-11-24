use anyhow::Result;
use dialoguer::{theme::ColorfulTheme, Input};

pub fn prompt(prompt: &str) -> Result<String> {
    Ok(Input::with_theme(&ColorfulTheme::default())
        .with_prompt(prompt)
        .interact()?)
}
