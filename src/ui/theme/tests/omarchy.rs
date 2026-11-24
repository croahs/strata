// SPDX-License-Identifier: MIT

use std::path::Path;

use super::{scheme_color_values, wait_for_live_theme};
use crate::test_support::gtk_test;
use crate::ui::theme::{
    OmarchyVariant, SourcePalette, ThemeManager, omarchy_state_dir, resolved_source_palette,
    source_palette_from_quattro, source_style_scheme_xml, tokens_from_quattro,
};

#[test]
fn theme_set_signal_reads_switched_and_edited_omarchy_palettes() {
    gtk_test(
        "ui::theme::tests::omarchy::theme_set_signal_reads_switched_and_edited_omarchy_palettes",
        || {
            let state = omarchy_state_dir();
            let themes = gtk::glib::home_dir().join("palettes");
            let write = |folder: &Path, accent: &str, atomic: bool| {
                std::fs::create_dir_all(folder).expect("palette directory");
                let destination = folder.join("colors.toml");
                let path = if atomic {
                    folder.join("colors.new")
                } else {
                    destination.clone()
                };
                std::fs::write(
                    &path,
                    format!(
                        "background = '#112233'\nforeground = '#ddeeff'\naccent = '{accent}'\n"
                    ),
                )
                .expect("palette written");
                if atomic {
                    std::fs::rename(path, destination).expect("atomic palette replacement");
                }
            };
            let switch = |name: &str| {
                let pending = state.join("next-theme");
                std::os::unix::fs::symlink(themes.join(name), &pending).expect("new theme link");
                std::fs::rename(pending, state.join("theme")).expect("atomic theme switch");
            };
            let theme_set = || {
                use rustix::process::{Signal, getpid, kill_process};
                kill_process(getpid(), Signal::USR2).expect("theme-set hook signal");
            };
            std::fs::create_dir_all(&state).expect("Omarchy state");
            std::fs::write(state.join("theme.name"), "fixture").expect("theme name");
            write(&themes.join("a"), "#112244", false);
            write(&themes.join("b"), "#445566", false);
            switch("a");
            let manager = ThemeManager::shared();
            manager.set_follow_omarchy(true);
            assert_eq!(manager.active_model_palette().accent, 0x11_22_44);

            for (name, initial, edited, color) in [
                ("b", 0x44_55_66, "#778899", 0x77_88_99),
                ("a", 0x11_22_44, "#aabbcc", 0xaa_bb_cc),
            ] {
                switch(name);
                theme_set();
                wait_for_live_theme(|| manager.active_model_palette().accent == initial);
                write(&themes.join(name), edited, false);
                theme_set();
                wait_for_live_theme(|| manager.active_model_palette().accent == color);
                write(&themes.join(name), "#abcdef", true);
                theme_set();
                wait_for_live_theme(|| manager.active_model_palette().accent == 0xab_cd_ef);
            }
        },
    );
}

#[test]
fn quattro_colors_map_to_strata_tokens() {
    let theme = tokens_from_quattro(
        "azure-glow",
        r##"
background = "#0a0f1a"
dark_background = "#070b13"
lighter_background = "#142033"
foreground = "#a8dfff"
accent = "#00aaff"
selection = "#1d3350"
muted = "#3a5068"
color8 = "#99aabb"
"##,
        OmarchyVariant::Lighter,
    )
    .expect("valid Quattro colors should map");

    for (token, actual, expected) in [
        ("name", theme.name.as_str(), "Azure Glow"),
        ("background", theme.background.as_str(), "#0a0f1a"),
        ("surface", theme.surface.as_str(), "#070b13"),
        ("muted", theme.muted.as_str(), "#142033"),
        ("highlight", theme.highlight.as_str(), "#1d3350"),
        ("border", theme.border.as_str(), "#3a5068"),
        ("accent", theme.accent.as_str(), "#00aaff"),
    ] {
        assert_eq!(actual, expected, "{token}");
    }

    let minimal = tokens_from_quattro(
        "azure-glow",
        "background = \"#0a0f1a\"\nlighter_background = \"#0a0f1a\"\nforeground = \"#a8dfff\"\naccent = \"#00aaff\"\ncolor8 = \"#99aabb\"\n",
        OmarchyVariant::Lighter,
    )
    .expect("required Quattro colors should map");
    assert_eq!(minimal.background, "#0a0f1a");
    assert_eq!(minimal.surface, "#0a0f1a");
    assert_ne!(minimal.muted, minimal.background);
    let derived = resolved_source_palette(&theme, None);
    for (source, expected) in [
        ("color2 = \"#00ff00\"", "#00ff00"),
        ("color2 = \"#00ff00\"\ngreen = \"#009900\"", "#009900"),
        ("green = \"invalid\"", derived.string.as_str()),
    ] {
        let source = format!(
            "background = \"#0a0f1a\"\nforeground = \"#a8dfff\"\naccent = \"#00aaff\"\nselection = \"#a8dfff\"\ncolor8 = \"#123247\"\n{source}"
        );
        let tokens = tokens_from_quattro("azure-glow", &source, OmarchyVariant::Lighter)
            .expect("partial Quattro palette");
        let palette = resolved_source_palette(&tokens, None);
        assert_eq!(palette.string, expected);
        assert_eq!(palette.statement, derived.statement);
    }
}

#[test]
fn quattro_syntax_colors_remain_theme_native() {
    let palette = source_palette_from_quattro(
        r##"
blue = "#111111"
cyan = "#222222"
green = "#333333"
yellow = "#444444"
orange = "#555555"
magenta = "#666666"
"##,
    )
    .expect("complete Quattro syntax palette");

    for (role, actual, expected) in [
        ("statement", palette.statement.as_str(), "#666666"),
        ("string", palette.string.as_str(), "#333333"),
        ("constant", palette.constant.as_str(), "#555555"),
        ("type", palette.type_color.as_str(), "#222222"),
        ("preprocessor", palette.preprocessor.as_str(), "#444444"),
    ] {
        assert_eq!(actual, expected, "{role}");
    }
}

#[test]
fn quattro_syntax_palette_treats_unparsable_colors_as_missing() {
    let base = [
        ("blue", "#111111"),
        ("cyan", "#222222"),
        ("green", "#333333"),
        ("yellow", "#444444"),
        ("orange", "#555555"),
        ("magenta", "#666666"),
    ];
    let source_with = |key: &str, value: &str| {
        base.iter()
            .map(|(name, color)| {
                let color = if *name == key { value } else { color };
                format!("{name} = \"{color}\"\n")
            })
            .collect::<String>()
    };
    type Field = fn(&SourcePalette) -> &str;
    let statement: Field = |palette| &palette.statement;
    let string: Field = |palette| &palette.string;
    let constant: Field = |palette| &palette.constant;
    let type_color: Field = |palette| &palette.type_color;
    for (key, value, expected) in [
        ("green", "invalid", None),
        ("yellow", "invalid", None),
        ("magenta", "0x666666", Some((statement, "#111111"))),
        ("orange", "notacolor", Some((constant, "#444444"))),
        ("cyan", "", Some((type_color, "#111111"))),
        ("green", "#333333333", Some((string, "#333333"))),
        (
            "magenta",
            "rgba(102,102,102,0.5)",
            Some((statement, "#66666680")),
        ),
    ] {
        let palette = source_palette_from_quattro(&source_with(key, value));
        match expected {
            None => assert!(palette.is_none(), "{key} = {value:?}"),
            Some((field, color)) => assert_eq!(
                palette.as_ref().map(field),
                Some(color),
                "{key} = {value:?}"
            ),
        }
    }

    let source = format!(
        "background = \"#0a0f1a\"\nforeground = \"#a8dfff\"\naccent = \"#00aaff\"\n{}",
        source_with("orange", "0x555555").replace("#666666", "notacolor")
    );
    let tokens = tokens_from_quattro("azure-glow", &source, OmarchyVariant::Lighter)
        .expect("required Quattro colors are valid");
    let xml = source_style_scheme_xml(&tokens, source_palette_from_quattro(&source).as_ref());
    let values = scheme_color_values(&xml);
    assert_eq!(values.len(), 12);
    for value in values {
        assert!(
            value.starts_with('#') && value.len() == 7,
            "scheme colors must be canonical #rrggbb, got {value}"
        );
    }
}

#[test]
fn legacy_palette_without_quattro_semantics_is_not_detected() {
    assert!(
        tokens_from_quattro("legacy", "color4 = \"#00aaff\"", OmarchyVariant::Lighter).is_none()
    );
}
