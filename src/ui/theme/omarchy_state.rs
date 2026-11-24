// SPDX-License-Identifier: MIT

//! Reading Omarchy's current theme state and turning its Quattro colors into
//! Strata tokens.

use std::{fs, path::PathBuf};

use super::{
    super::preferences::OmarchyVariant, SourcePalette, ThemeTokens, blend, canonical_color,
    default_danger, omarchy,
};

#[derive(Clone, PartialEq)]
pub(super) struct OmarchyState {
    pub(super) name: String,
    pub(super) colors: String,
}

impl OmarchyState {
    pub(super) fn tokens(&self, variant: OmarchyVariant) -> Option<ThemeTokens> {
        tokens_from_quattro(self.name.trim(), &self.colors, variant)
    }

    pub(super) fn source_palette(&self) -> Option<SourcePalette> {
        source_palette_from_quattro(&self.colors)
    }
}

pub(super) struct OmarchyRefresh {
    pub(super) state: Option<OmarchyState>,
    pub(super) name_exists: bool,
}

impl OmarchyRefresh {
    pub(super) fn read() -> Self {
        let state = omarchy_state_dir();
        let read = |name: &str| fs::read_to_string(state.join(name)).ok();
        Self {
            state: read("theme.name")
                .zip(read("theme/colors.toml"))
                .map(|(name, colors)| OmarchyState { name, colors }),
            name_exists: state.join("theme.name").is_file(),
        }
    }
}

pub(super) fn tokens_from_quattro(
    name: &str,
    source: &str,
    variant: OmarchyVariant,
) -> Option<ThemeTokens> {
    let values: toml::Value = toml::from_str(source).ok()?;
    let get = |key: &str| quattro_color(&values, key);
    let source_background = get("background")?;
    let text = get("foreground")?;
    let accent = get("accent")?;
    let selection = get("selection").unwrap_or_else(|| blend(&source_background, &accent, 0.25));
    // Some themes set lighter_background equal to background, which would hide hover.
    let lighter = get("lighter_background")
        .filter(|lighter| *lighter != source_background)
        .unwrap_or_else(|| blend(&source_background, &text, 0.10));
    let mut tokens = ThemeTokens {
        name: title_case_slug(name),
        background: source_background.clone(),
        surface: get("dark_background").unwrap_or_else(|| source_background.clone()),
        muted: lighter,
        highlight: selection,
        border: get("muted").unwrap_or_else(|| blend(&source_background, &text, 0.36)),
        dim_text: blend(&source_background, &text, 0.62),
        text,
        accent,
        danger: get("color1").unwrap_or_else(default_danger),
        syntax_keyword: get("magenta").or_else(|| get("color5")),
        syntax_string: get("green").or_else(|| get("color2")),
        syntax_constant: get("orange").or_else(|| get("color9")),
        syntax_type: get("cyan").or_else(|| get("color3")),
        syntax_preprocessor: get("yellow"),
    };
    let light = omarchy::is_light(
        &source_background,
        values.get("mode").and_then(toml::Value::as_str),
    );
    let palette = source_palette_from_quattro(source);
    omarchy::apply_variant(&mut tokens, light, variant, palette.as_ref());
    Some(tokens)
}

pub(super) fn source_palette_from_quattro(source: &str) -> Option<SourcePalette> {
    let values: toml::Value = toml::from_str(source).ok()?;
    let get = |key: &str| quattro_color(&values, key);
    Some(SourcePalette {
        statement: get("magenta").or_else(|| get("blue"))?,
        string: get("green")?,
        constant: get("orange").or_else(|| get("yellow"))?,
        type_color: get("cyan").or_else(|| get("blue"))?,
        preprocessor: get("yellow")?,
    })
}

/// Quattro values reach GTK CSS, GtkSourceView XML and SVG strokes, so a value GTK
/// cannot parse is treated as missing; callers decide whether that is fatal.
fn quattro_color(values: &toml::Value, key: &str) -> Option<String> {
    canonical_color(values.get(key)?.as_str()?)
}

pub(super) fn title_case_slug(slug: &str) -> String {
    slug.split('-')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            chars
                .next()
                .map(|first| first.to_uppercase().collect::<String>() + chars.as_str())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub(in crate::ui) fn omarchy_state_dir() -> PathBuf {
    gtk::glib::home_dir().join(".local/state/omarchy/current")
}
