use anyhow::Result;
use dialoguer::{theme::ColorfulTheme, Confirm, Input, Select};

pub fn prompt(prompt: &str) -> Result<String> {
    Ok(Input::with_theme(&ColorfulTheme::default())
        .with_prompt(prompt)
        .interact()?)
}

pub fn prompt_yes_no(prompt: &str) -> Result<bool> {
    Ok(Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt(prompt)
        .interact()?)
}

pub fn prompt_select(prompt: &str, options: &[&str], default: usize) -> Result<usize> {
    Ok(Select::with_theme(&ColorfulTheme::default())
        .with_prompt(prompt)
        .items(&options)
        .default(default)
        .interact()?)
}
