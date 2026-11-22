//! Theme presets and custom color validation.
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub struct Theme {
    background: String,
    text: String,
    accent: String,
}

#[derive(Deserialize)]
#[serde(untagged)]
pub(super) enum ThemeSpec {
    Preset(String),
    Custom(Colors),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Colors {
    base: Option<String>,
    background: Option<String>,
    text: Option<String>,
    accent: Option<String>,
}

impl Default for ThemeSpec {
    fn default() -> Self {
        Self::Preset("light".into())
    }
}

impl Theme {
    fn preset(name: &str) -> Result<Self, String> {
        let (background, text, accent) = match name {
            "light" => ("#f8f9fc", "#172239", "#254eb4"),
            "midnight" => ("#101827", "#f2f5fc", "#9ebaff"),
            _ => return Err(format!("Unknown theme {name:?}; use light or midnight")),
        };
        Ok(Self {
            background: background.into(),
            text: text.into(),
            accent: accent.into(),
        })
    }

    pub(super) fn resolve(spec: ThemeSpec) -> Result<Self, String> {
        match spec {
            ThemeSpec::Preset(name) => Self::preset(&name),
            ThemeSpec::Custom(colors) => {
                let mut theme = Self::preset(colors.base.as_deref().unwrap_or("light"))?;
                for (name, target, value) in [
                    ("background", &mut theme.background, colors.background),
                    ("text", &mut theme.text, colors.text),
                    ("accent", &mut theme.accent, colors.accent),
                ] {
                    if let Some(value) = value {
                        let Some(hex) = value.strip_prefix('#') else {
                            return Err(format!("Theme {name} must be a quoted hex color"));
                        };
                        if !matches!(hex.len(), 3 | 6)
                            || !hex.bytes().all(|c| c.is_ascii_hexdigit())
                        {
                            return Err(format!("Theme {name} must be #RGB or #RRGGBB"));
                        }
                        *target = value;
                    }
                }
                Ok(theme)
            }
        }
    }
}
