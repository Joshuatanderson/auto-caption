use std::{collections::HashMap, path::PathBuf, sync::Mutex};

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::pipeline::types::CaptionPosition;

pub struct DbState(pub Mutex<Connection>);

#[derive(Serialize, Deserialize)]
pub struct ThemeMeta {
    pub slug: String,
    pub name: String,
    pub swatch: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct AssStyleColors {
    pub primary_color: String,
    pub accent_color: String,
    /// Crisp stroke around every glyph. Theme-controlled so each theme can
    /// "bring its own contrast" against any video background (light text wants
    /// a dark edge; dark text wants a light edge) rather than relying on the
    /// footage being a flattering color. `#[serde(default)]` keeps pre-outline
    /// rows (e.g. an existing 'custom' theme) parsing — they fall back to black.
    #[serde(default = "default_outline_color")]
    pub outline_color: String,
    /// Optional opaque "chip" box behind the active word (the high-pop karaoke
    /// look). Empty string = no box (classic glow+highlight). `#[serde(default)]`
    /// → "" so pre-box rows (old themes, 'custom') still parse.
    #[serde(default)]
    pub box_color: String,
}

fn default_outline_color() -> String {
    "&H00000000".to_string()
}

#[derive(Serialize, Deserialize)]
pub struct ThemeData {
    pub slug: String,
    pub name: String,
    pub css_vars: HashMap<String, String>,
    pub ass_style: AssStyleColors,
}

pub fn init(path: PathBuf) -> rusqlite::Result<Connection> {
    let conn = Connection::open(path)?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS themes (
            slug      TEXT PRIMARY KEY,
            name      TEXT NOT NULL,
            swatch    TEXT NOT NULL,
            css_vars  TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS user_preferences (
            id            INTEGER PRIMARY KEY CHECK (id = 1),
            current_theme TEXT NOT NULL DEFAULT 'happycampr'
        );
        CREATE TABLE IF NOT EXISTS dep_paths (
            name TEXT PRIMARY KEY,
            path TEXT NOT NULL
        );",
    )?;
    // Migration: add ass_style column if missing (error means it already exists — ignore it)
    let _ = conn.execute("ALTER TABLE themes ADD COLUMN ass_style TEXT", []);
    let _ = conn.execute("ALTER TABLE user_preferences ADD COLUMN output_dir TEXT", []);
    let _ = conn.execute(
        "ALTER TABLE user_preferences ADD COLUMN caption_position TEXT NOT NULL DEFAULT 'bottom'",
        [],
    );
    seed_themes(&conn)?;
    conn.execute(
        "INSERT OR IGNORE INTO user_preferences (id, current_theme) VALUES (1, 'happycampr')",
        [],
    )?;
    // Migration: retire themes that are no longer seeded. seed_themes only
    // INSERT-OR-REPLACEs the slugs it lists, so dropped slugs would otherwise
    // linger as stale rows. Retired: 'cantaloupe' (old branding) and the muted
    // glow set ('meadow','trail','espresso'), superseded by the bright box themes.
    // Repoint anyone still on a retired theme to the green default.
    let _ = conn.execute(
        "DELETE FROM themes WHERE slug IN ('cantaloupe','meadow','trail','espresso')",
        [],
    );
    let _ = conn.execute(
        "UPDATE user_preferences SET current_theme = 'happycampr' \
         WHERE current_theme IN ('cantaloupe','meadow','trail','espresso')",
        [],
    );
    Ok(conn)
}

fn seed_themes(conn: &Connection) -> rusqlite::Result<()> {
    // INSERT OR REPLACE so color updates always propagate on restart
    let themes: &[(&str, &str, &str, serde_json::Value, AssStyleColors)] = &[
        // ── Happycampr brand themes ──────────────────────────────────────────
        // Palette (Mimir design tokens): marshmallow #F5F1E8 · moss #5A6B4C ·
        // graham #946334 · burnt #2B1810. ASS colors are &H00BBGGRR (BGR order):
        //   marshmallow → &H00E8F1F5   moss   → &H004C6B5A
        //   graham      → &H00346394   burnt  → &H0010182B   black → &H00000000
        // Every theme pairs a light and a dark brand color across text / accent
        // / outline so the caption stays WCAG-legible on any footage without
        // leaning on the background.
        (
            "happycampr",
            "Happycampr",
            // moss chip (box) + cream word
            "linear-gradient(135deg, #5A6B4C 0% 60%, #F5F1E8 60% 100%)",
            serde_json::json!({
                "--background":            "oklch(0.97 0.012 87)",
                "--foreground":            "oklch(0.23 0.034 43.5)",
                "--card":                  "oklch(0.98 0.008 87)",
                "--card-foreground":       "oklch(0.23 0.034 43.5)",
                "--popover":               "oklch(0.98 0.008 87)",
                "--popover-foreground":    "oklch(0.23 0.034 43.5)",
                "--primary":               "oklch(0.5048 0.0516 131.41)",
                "--primary-foreground":    "oklch(0.97 0.012 87)",
                "--secondary":             "oklch(0.92 0.022 120)",
                "--secondary-foreground":  "oklch(0.36 0.045 131)",
                "--muted":                 "oklch(0.93 0.015 90)",
                "--muted-foreground":      "oklch(0.48 0.03 55)",
                "--accent":                "oklch(0.90 0.032 125)",
                "--accent-foreground":     "oklch(0.36 0.045 131)",
                "--destructive":           "oklch(0.577 0.245 27.325)",
                "--border":                "oklch(0.88 0.02 100)",
                "--input":                 "oklch(0.88 0.02 100)",
                "--ring":                  "oklch(0.5048 0.0516 131.41)",
                "--sidebar":               "oklch(0.95 0.013 90)",
                "--sidebar-foreground":    "oklch(0.23 0.034 43.5)",
                "--sidebar-primary":       "oklch(0.5048 0.0516 131.41)",
                "--sidebar-primary-foreground": "oklch(0.97 0.012 87)",
                "--sidebar-accent":        "oklch(0.90 0.032 125)",
                "--sidebar-accent-foreground": "oklch(0.36 0.045 131)",
                "--sidebar-border":        "oklch(0.88 0.02 100)",
                "--sidebar-ring":          "oklch(0.5048 0.0516 131.41)"
            }),
            // BOX style — near-white text, burnt edge, moss chip behind the active
            // word with a cream word inside. word/chip 5.1:1 (AA). The on-brand default.
            AssStyleColors {
                primary_color: "&H00F6FAFB".to_string(),
                accent_color: "&H00E8F1F5".to_string(),
                outline_color: "&H0010182B".to_string(),
                box_color: "&H004C6B5A".to_string(),
            },
        ),
        (
            "graham",
            "Graham",
            // graham chip (box) + cream word
            "linear-gradient(135deg, #946334 0% 60%, #F5F1E8 60% 100%)",
            serde_json::json!({
                "--background":            "oklch(0.97 0.014 80)",
                "--foreground":            "oklch(0.23 0.034 43.5)",
                "--card":                  "oklch(0.98 0.01 80)",
                "--card-foreground":       "oklch(0.23 0.034 43.5)",
                "--popover":               "oklch(0.98 0.01 80)",
                "--popover-foreground":    "oklch(0.23 0.034 43.5)",
                "--primary":               "oklch(0.5423 0.0893 62.79)",
                "--primary-foreground":    "oklch(0.97 0.012 87)",
                "--secondary":             "oklch(0.93 0.03 72)",
                "--secondary-foreground":  "oklch(0.38 0.06 62)",
                "--muted":                 "oklch(0.93 0.02 75)",
                "--muted-foreground":      "oklch(0.48 0.04 60)",
                "--accent":                "oklch(0.91 0.04 68)",
                "--accent-foreground":     "oklch(0.38 0.06 62)",
                "--destructive":           "oklch(0.577 0.245 27.325)",
                "--border":                "oklch(0.88 0.025 72)",
                "--input":                 "oklch(0.88 0.025 72)",
                "--ring":                  "oklch(0.5423 0.0893 62.79)",
                "--sidebar":               "oklch(0.95 0.018 76)",
                "--sidebar-foreground":    "oklch(0.23 0.034 43.5)",
                "--sidebar-primary":       "oklch(0.5423 0.0893 62.79)",
                "--sidebar-primary-foreground": "oklch(0.97 0.012 87)",
                "--sidebar-accent":        "oklch(0.91 0.04 68)",
                "--sidebar-accent-foreground": "oklch(0.38 0.06 62)",
                "--sidebar-border":        "oklch(0.88 0.025 72)",
                "--sidebar-ring":          "oklch(0.5423 0.0893 62.79)"
            }),
            // BOX style — near-white text, burnt edge, caramel (graham) chip behind
            // the active word with a cream word inside. word/chip 4.6:1 (AA).
            AssStyleColors {
                primary_color: "&H00F6FAFB".to_string(),
                accent_color: "&H00E8F1F5".to_string(),
                outline_color: "&H0010182B".to_string(),
                box_color: "&H00346394".to_string(),
            },
        ),
        (
            "gold",
            "Gold",
            // burnt chip (box) + bright gold word
            "linear-gradient(135deg, #2B1810 0% 60%, #FFC53D 60% 100%)",
            serde_json::json!({
                "--background":            "oklch(0.19 0.018 50)",
                "--foreground":            "oklch(0.95 0.02 90)",
                "--card":                  "oklch(0.23 0.02 50)",
                "--card-foreground":       "oklch(0.95 0.02 90)",
                "--popover":               "oklch(0.23 0.02 50)",
                "--popover-foreground":    "oklch(0.95 0.02 90)",
                "--primary":               "oklch(0.85 0.16 85)",
                "--primary-foreground":    "oklch(0.18 0.02 50)",
                "--secondary":             "oklch(0.27 0.025 55)",
                "--secondary-foreground":  "oklch(0.95 0.02 90)",
                "--muted":                 "oklch(0.27 0.025 55)",
                "--muted-foreground":      "oklch(0.78 0.06 85)",
                "--accent":                "oklch(0.30 0.03 60)",
                "--accent-foreground":     "oklch(0.95 0.02 90)",
                "--destructive":           "oklch(0.704 0.191 22.216)",
                "--border":                "oklch(1 0 0 / 12%)",
                "--input":                 "oklch(1 0 0 / 15%)",
                "--ring":                  "oklch(0.85 0.16 85)",
                "--sidebar":               "oklch(0.23 0.02 50)",
                "--sidebar-foreground":    "oklch(0.95 0.02 90)",
                "--sidebar-primary":       "oklch(0.85 0.16 85)",
                "--sidebar-primary-foreground": "oklch(0.18 0.02 50)",
                "--sidebar-accent":        "oklch(0.30 0.03 60)",
                "--sidebar-accent-foreground": "oklch(0.95 0.02 90)",
                "--sidebar-border":        "oklch(1 0 0 / 12%)",
                "--sidebar-ring":          "oklch(0.85 0.16 85)"
            }),
            // BOX style — near-white text, burnt chip behind the active word with a
            // bright gold word inside. word/chip 10.7:1 (AAA). Closest to the classic
            // creator-caption look, kept on Happycampr's warm side.
            AssStyleColors {
                primary_color: "&H00F6FAFB".to_string(),
                accent_color: "&H003DC5FF".to_string(),
                outline_color: "&H0010182B".to_string(),
                box_color: "&H0010182B".to_string(),
            },
        ),
        (
            "obsidian",
            "Obsidian",
            "oklch(0.13 0.005 265)",
            serde_json::json!({
                "--background":            "oklch(0.13 0.005 265)",
                "--foreground":            "oklch(0.93 0.01 265)",
                "--card":                  "oklch(0.18 0.008 265)",
                "--card-foreground":       "oklch(0.93 0.01 265)",
                "--popover":               "oklch(0.18 0.008 265)",
                "--popover-foreground":    "oklch(0.93 0.01 265)",
                "--primary":               "oklch(0.65 0.15 285)",
                "--primary-foreground":    "oklch(0.98 0 0)",
                "--secondary":             "oklch(0.22 0.01 265)",
                "--secondary-foreground":  "oklch(0.93 0.01 265)",
                "--muted":                 "oklch(0.22 0.01 265)",
                "--muted-foreground":      "oklch(0.65 0.02 265)",
                "--accent":                "oklch(0.22 0.01 265)",
                "--accent-foreground":     "oklch(0.93 0.01 265)",
                "--destructive":           "oklch(0.704 0.191 22.216)",
                "--border":                "oklch(1 0 0 / 12%)",
                "--input":                 "oklch(1 0 0 / 15%)",
                "--ring":                  "oklch(0.65 0.15 285)",
                "--sidebar":               "oklch(0.18 0.008 265)",
                "--sidebar-foreground":    "oklch(0.93 0.01 265)",
                "--sidebar-primary":       "oklch(0.65 0.15 285)",
                "--sidebar-primary-foreground": "oklch(0.98 0 0)",
                "--sidebar-accent":        "oklch(0.22 0.01 265)",
                "--sidebar-accent-foreground": "oklch(0.93 0.01 265)",
                "--sidebar-border":        "oklch(1 0 0 / 12%)",
                "--sidebar-ring":          "oklch(0.65 0.15 285)"
            }),
            // Captions: white text, violet accent (#8B5CF6 → ASS BBGGRR = F65C8B). Glow style.
            AssStyleColors {
                primary_color: "&H00FFFFFF".to_string(),
                accent_color: "&H00F65C8B".to_string(),
                outline_color: "&H00000000".to_string(),
                box_color: String::new(),
            },
        ),
        (
            "yellow",
            "High Contrast",
            "oklch(0.87 0.20 95)",
            serde_json::json!({
                "--background":            "oklch(0.99 0 0)",
                "--foreground":            "oklch(0.08 0.01 100)",
                "--card":                  "oklch(0.99 0 0)",
                "--card-foreground":       "oklch(0.08 0.01 100)",
                "--popover":               "oklch(0.99 0 0)",
                "--popover-foreground":    "oklch(0.08 0.01 100)",
                "--primary":               "oklch(0.87 0.20 95)",
                "--primary-foreground":    "oklch(0.08 0.01 100)",
                "--secondary":             "oklch(0.94 0.06 95)",
                "--secondary-foreground":  "oklch(0.08 0.01 100)",
                "--muted":                 "oklch(0.94 0.06 95)",
                "--muted-foreground":      "oklch(0.40 0.02 95)",
                "--accent":                "oklch(0.94 0.06 95)",
                "--accent-foreground":     "oklch(0.08 0.01 100)",
                "--destructive":           "oklch(0.577 0.245 27.325)",
                "--border":                "oklch(0.12 0.02 100)",
                "--input":                 "oklch(0.88 0.08 95)",
                "--ring":                  "oklch(0.87 0.20 95)",
                "--sidebar":               "oklch(0.96 0.04 95)",
                "--sidebar-foreground":    "oklch(0.08 0.01 100)",
                "--sidebar-primary":       "oklch(0.87 0.20 95)",
                "--sidebar-primary-foreground": "oklch(0.08 0.01 100)",
                "--sidebar-accent":        "oklch(0.94 0.06 95)",
                "--sidebar-accent-foreground": "oklch(0.08 0.01 100)",
                "--sidebar-border":        "oklch(0.12 0.02 100)",
                "--sidebar-ring":          "oklch(0.87 0.20 95)"
            }),
            // Captions: white text, bright yellow accent (the original default look). Glow style.
            AssStyleColors {
                primary_color: "&H00FFFFFF".to_string(),
                accent_color: "&H0000FFFF".to_string(),
                outline_color: "&H00000000".to_string(),
                box_color: String::new(),
            },
        ),
    ];

    for (slug, name, swatch, css_vars, ass_style) in themes {
        let ass_json = serde_json::to_string(ass_style).unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO themes (slug, name, swatch, css_vars, ass_style)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![slug, name, swatch, css_vars.to_string(), ass_json],
        )?;
    }

    // 'custom' row is user-editable — INSERT OR IGNORE so customizations persist.
    // CSS vars mirror happycampr so the surrounding UI stays coherent; caption
    // colors start at white+yellow and are overridden via set_custom_ass_colors.
    let custom_css_vars = themes
        .iter()
        .find(|(s, ..)| *s == "happycampr")
        .map(|(_, _, _, v, _)| v.to_string())
        .unwrap_or_else(|| "{}".to_string());
    let custom_ass = AssStyleColors {
        primary_color: "&H00FFFFFF".to_string(),
        accent_color: "&H0000FFFF".to_string(),
        outline_color: "&H00000000".to_string(),
        box_color: String::new(),
    };
    conn.execute(
        "INSERT OR IGNORE INTO themes (slug, name, swatch, css_vars, ass_style)
         VALUES ('custom', 'Custom', ?1, ?2, ?3)",
        params![
            "linear-gradient(135deg, #FFFF00 0% 50%, #FFFFFF 50% 100%)",
            custom_css_vars,
            serde_json::to_string(&custom_ass).unwrap(),
        ],
    )?;
    Ok(())
}

/// Converts "#RRGGBB" to ASS "&H00BBGGRR" (opaque, little-endian channels).
pub fn hex_to_ass(hex: &str) -> Option<String> {
    let s = hex.trim().strip_prefix('#')?;
    if s.len() != 6 || !s.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let rr = &s[0..2];
    let gg = &s[2..4];
    let bb = &s[4..6];
    Some(format!(
        "&H00{}{}{}",
        bb.to_uppercase(),
        gg.to_uppercase(),
        rr.to_uppercase()
    ))
}

/// Converts ASS "&HaaBBGGRR" (any alpha prefix) to "#RRGGBB" for an <input type="color">.
pub fn ass_to_hex(ass: &str) -> Option<String> {
    let s = ass
        .trim()
        .strip_prefix("&H")
        .or_else(|| ass.trim().strip_prefix("&h"))?;
    if s.len() < 6 {
        return None;
    }
    let tail = &s[s.len() - 6..];
    if !tail.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let bb = &tail[0..2];
    let gg = &tail[2..4];
    let rr = &tail[4..6];
    Some(format!(
        "#{}{}{}",
        rr.to_uppercase(),
        gg.to_uppercase(),
        bb.to_uppercase()
    ))
}

/// Returns the ASS caption colors for the currently selected theme.
/// Called by the generate_ass command — no Tauri State needed, takes a raw Connection.
pub fn current_ass_style(conn: &Connection) -> AssStyleColors {
    let slug: String = conn
        .query_row(
            "SELECT current_theme FROM user_preferences WHERE id = 1",
            [],
            |row| row.get(0),
        )
        .unwrap_or_else(|_| "happycampr".to_string());

    conn.query_row(
        "SELECT ass_style FROM themes WHERE slug = ?1",
        params![slug],
        |row| row.get::<_, Option<String>>(0),
    )
    .ok()
    .flatten()
    .and_then(|s| serde_json::from_str::<AssStyleColors>(&s).ok())
    .unwrap_or_else(|| AssStyleColors {
        primary_color: "&H00FFFFFF".to_string(),
        accent_color: "&H0000FFFF".to_string(),
        outline_color: "&H00000000".to_string(),
        box_color: String::new(),
    })
}

/// Fixed display order for the theme picker. Built-ins sort by this list;
/// rowid can't be trusted because INSERT-OR-REPLACE reseeding and the
/// Cantaloupe→Happycampr migration reshuffle it. Unknown slugs fall after the
/// known ones (but before 'custom', which is pinned last by its high index).
fn theme_sort_index(slug: &str) -> usize {
    const ORDER: &[&str] = &[
        "happycampr", "graham", "gold", "obsidian", "yellow", "custom",
    ];
    ORDER
        .iter()
        .position(|s| *s == slug)
        .unwrap_or(ORDER.len() - 1)
}

#[tauri::command]
pub fn get_themes(state: tauri::State<DbState>) -> Result<Vec<ThemeMeta>, String> {
    let db = state.0.lock().map_err(|e| e.to_string())?;
    let mut stmt = db
        .prepare("SELECT slug, name, swatch FROM themes")
        .map_err(|e| e.to_string())?;
    let mut themes = stmt
        .query_map([], |row| {
            Ok(ThemeMeta {
                slug: row.get(0)?,
                name: row.get(1)?,
                swatch: row.get(2)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    themes.sort_by_key(|t| theme_sort_index(&t.slug));
    Ok(themes)
}

#[tauri::command]
pub fn get_current_theme(state: tauri::State<DbState>) -> Result<ThemeData, String> {
    let db = state.0.lock().map_err(|e| e.to_string())?;
    let slug: String = db
        .query_row(
            "SELECT current_theme FROM user_preferences WHERE id = 1",
            [],
            |row| row.get(0),
        )
        .unwrap_or_else(|_| "happycampr".to_string());

    let (name, css_vars_str, ass_style_str): (String, String, Option<String>) = db
        .query_row(
            "SELECT name, css_vars, ass_style FROM themes WHERE slug = ?1",
            params![slug],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .map_err(|e| e.to_string())?;

    let css_vars: HashMap<String, String> =
        serde_json::from_str(&css_vars_str).map_err(|e| e.to_string())?;

    let ass_style = ass_style_str
        .and_then(|s| serde_json::from_str::<AssStyleColors>(&s).ok())
        .unwrap_or_else(|| AssStyleColors {
            primary_color: "&H00FFFFFF".to_string(),
            accent_color: "&H0000FFFF".to_string(),
            outline_color: "&H00000000".to_string(),
            box_color: String::new(),
        });

    Ok(ThemeData {
        slug,
        name,
        css_vars,
        ass_style,
    })
}

#[tauri::command]
pub fn set_theme(slug: String, state: tauri::State<DbState>) -> Result<(), String> {
    let db = state.0.lock().map_err(|e| e.to_string())?;
    // Must be UPDATE, not INSERT OR REPLACE: the row has output_dir and
    // caption_position columns that aren't listed here, and REPLACE would
    // DELETE + re-INSERT, quietly wiping those values every theme switch.
    db.execute(
        "UPDATE user_preferences SET current_theme = ?1 WHERE id = 1",
        params![slug],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Reads the configured output directory, if any. None means "not configured —
/// fall back to input parent." Empty strings and paths that no longer exist
/// are NOT mapped to None here: callers need to know the difference so they
/// can fail loud on a deleted configured path instead of silently falling back.
pub fn current_output_dir(conn: &Connection) -> Option<PathBuf> {
    let raw: Option<String> = conn
        .query_row(
            "SELECT output_dir FROM user_preferences WHERE id = 1",
            [],
            |row| row.get(0),
        )
        .ok()
        .flatten();
    raw.and_then(|s| {
        let trimmed = s.trim();
        if trimmed.is_empty() { None } else { Some(PathBuf::from(trimmed)) }
    })
}

#[tauri::command]
pub fn get_output_dir(state: tauri::State<DbState>) -> Result<Option<String>, String> {
    let db = state.0.lock().map_err(|e| e.to_string())?;
    Ok(current_output_dir(&db).map(|p| p.to_string_lossy().into_owned()))
}

#[tauri::command]
pub fn set_output_dir(
    path: Option<String>,
    state: tauri::State<DbState>,
) -> Result<(), String> {
    let db = state.0.lock().map_err(|e| e.to_string())?;
    let normalized = path.and_then(|s| {
        let t = s.trim().to_string();
        if t.is_empty() { None } else { Some(t) }
    });
    db.execute(
        "UPDATE user_preferences SET output_dir = ?1 WHERE id = 1",
        params![normalized],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Returns the user's selected caption vertical position. Defaults to Bottom
/// if the row or column is missing (fresh install / failed migration).
pub fn current_caption_position(conn: &Connection) -> CaptionPosition {
    let raw: Option<String> = conn
        .query_row(
            "SELECT caption_position FROM user_preferences WHERE id = 1",
            [],
            |row| row.get(0),
        )
        .ok();
    match raw.as_deref() {
        Some("top") => CaptionPosition::Top,
        Some("middle") => CaptionPosition::Middle,
        _ => CaptionPosition::Bottom,
    }
}

#[tauri::command]
pub fn get_caption_position(state: tauri::State<DbState>) -> Result<CaptionPosition, String> {
    let db = state.0.lock().map_err(|e| e.to_string())?;
    Ok(current_caption_position(&db))
}

#[tauri::command]
pub fn set_caption_position(
    position: CaptionPosition,
    state: tauri::State<DbState>,
) -> Result<(), String> {
    let db = state.0.lock().map_err(|e| e.to_string())?;
    let slug = match position {
        CaptionPosition::Top => "top",
        CaptionPosition::Middle => "middle",
        CaptionPosition::Bottom => "bottom",
    };
    db.execute(
        "UPDATE user_preferences SET caption_position = ?1 WHERE id = 1",
        params![slug],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[derive(Serialize, Deserialize)]
pub struct CustomAssColors {
    pub primary_hex: String,
    pub accent_hex: String,
}

#[tauri::command]
pub fn get_custom_ass_colors(
    state: tauri::State<DbState>,
) -> Result<CustomAssColors, String> {
    let db = state.0.lock().map_err(|e| e.to_string())?;
    let ass_json: Option<String> = db
        .query_row(
            "SELECT ass_style FROM themes WHERE slug = 'custom'",
            [],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    let ass = ass_json
        .and_then(|s| serde_json::from_str::<AssStyleColors>(&s).ok())
        .unwrap_or_else(|| AssStyleColors {
            primary_color: "&H00FFFFFF".to_string(),
            accent_color: "&H0000FFFF".to_string(),
            outline_color: "&H00000000".to_string(),
            box_color: String::new(),
        });
    Ok(CustomAssColors {
        primary_hex: ass_to_hex(&ass.primary_color).unwrap_or_else(|| "#FFFFFF".to_string()),
        accent_hex: ass_to_hex(&ass.accent_color).unwrap_or_else(|| "#FFFF00".to_string()),
    })
}

#[tauri::command]
pub fn set_custom_ass_colors(
    primary_hex: String,
    accent_hex: String,
    state: tauri::State<DbState>,
) -> Result<(), String> {
    let primary_ass = hex_to_ass(&primary_hex)
        .ok_or_else(|| format!("invalid primary hex: {primary_hex}"))?;
    let accent_ass = hex_to_ass(&accent_hex)
        .ok_or_else(|| format!("invalid accent hex: {accent_hex}"))?;
    let ass_json = serde_json::to_string(&AssStyleColors {
        primary_color: primary_ass,
        accent_color: accent_ass,
        // Custom captions keep the standard black edge (the picker exposes only
        // the inner/outer fill colors, not the stroke) and the glow style (no box).
        outline_color: "&H00000000".to_string(),
        box_color: String::new(),
    })
    .map_err(|e| e.to_string())?;
    let swatch = format!(
        "linear-gradient(135deg, {} 0% 50%, {} 50% 100%)",
        accent_hex.to_uppercase(),
        primary_hex.to_uppercase()
    );
    let db = state.0.lock().map_err(|e| e.to_string())?;
    db.execute(
        "UPDATE themes SET ass_style = ?1, swatch = ?2 WHERE slug = 'custom'",
        params![ass_json, swatch],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
