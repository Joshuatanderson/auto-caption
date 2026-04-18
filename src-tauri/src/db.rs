use std::{collections::HashMap, path::PathBuf, sync::Mutex};

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

pub struct DbState(pub Mutex<Connection>);

#[derive(Serialize, Deserialize)]
pub struct ThemeMeta {
    pub slug: String,
    pub name: String,
    pub swatch: String,
}

#[derive(Serialize, Deserialize)]
pub struct ThemeData {
    pub slug: String,
    pub name: String,
    pub css_vars: HashMap<String, String>,
}

pub fn init(path: PathBuf) -> rusqlite::Result<Connection> {
    let conn = Connection::open(path)?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS themes (
            slug     TEXT PRIMARY KEY,
            name     TEXT NOT NULL,
            swatch   TEXT NOT NULL,
            css_vars TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS user_preferences (
            id            INTEGER PRIMARY KEY CHECK (id = 1),
            current_theme TEXT NOT NULL DEFAULT 'cantaloupe'
        );",
    )?;
    seed_if_empty(&conn)?;
    Ok(conn)
}

fn seed_if_empty(conn: &Connection) -> rusqlite::Result<()> {
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM themes", [], |row| row.get(0))?;
    if count > 0 {
        return Ok(());
    }

    // Cantaloupe — brand green, white canvas aesthetic
    let cantaloupe = serde_json::json!({
        "--background":            "oklch(1 0 0)",
        "--foreground":            "oklch(0.17 0.01 264)",
        "--card":                  "oklch(1 0 0)",
        "--card-foreground":       "oklch(0.17 0.01 264)",
        "--popover":               "oklch(1 0 0)",
        "--popover-foreground":    "oklch(0.17 0.01 264)",
        "--primary":               "oklch(0.53 0.17 148)",
        "--primary-foreground":    "oklch(1 0 0)",
        "--secondary":             "oklch(0.97 0.04 152)",
        "--secondary-foreground":  "oklch(0.35 0.07 163)",
        "--muted":                 "oklch(0.97 0.04 152)",
        "--muted-foreground":      "oklch(0.52 0.01 264)",
        "--accent":                "oklch(0.94 0.08 152)",
        "--accent-foreground":     "oklch(0.35 0.07 163)",
        "--destructive":           "oklch(0.577 0.245 27.325)",
        "--border":                "oklch(0.94 0.03 152)",
        "--input":                 "oklch(0.94 0.03 152)",
        "--ring":                  "oklch(0.76 0.14 163)",
        "--sidebar":               "oklch(0.99 0.015 152)",
        "--sidebar-foreground":    "oklch(0.17 0.01 264)",
        "--sidebar-primary":       "oklch(0.53 0.17 148)",
        "--sidebar-primary-foreground": "oklch(1 0 0)",
        "--sidebar-accent":        "oklch(0.94 0.08 152)",
        "--sidebar-accent-foreground": "oklch(0.35 0.07 163)",
        "--sidebar-border":        "oklch(0.94 0.03 152)",
        "--sidebar-ring":          "oklch(0.76 0.14 163)"
    });

    // Obsidian — dark with cool violet primary
    let obsidian = serde_json::json!({
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
    });

    // High Contrast — bright yellow primary, very dark borders
    let yellow = serde_json::json!({
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
    });

    conn.execute(
        "INSERT INTO themes (slug, name, swatch, css_vars) VALUES (?1, ?2, ?3, ?4)",
        params!["cantaloupe", "Cantaloupe", "oklch(0.53 0.17 148)", cantaloupe.to_string()],
    )?;
    conn.execute(
        "INSERT INTO themes (slug, name, swatch, css_vars) VALUES (?1, ?2, ?3, ?4)",
        params!["obsidian", "Obsidian", "oklch(0.13 0.005 265)", obsidian.to_string()],
    )?;
    conn.execute(
        "INSERT INTO themes (slug, name, swatch, css_vars) VALUES (?1, ?2, ?3, ?4)",
        params!["yellow", "High Contrast", "oklch(0.87 0.20 95)", yellow.to_string()],
    )?;
    conn.execute(
        "INSERT OR IGNORE INTO user_preferences (id, current_theme) VALUES (1, 'cantaloupe')",
        [],
    )?;

    Ok(())
}

#[tauri::command]
pub fn get_themes(state: tauri::State<DbState>) -> Result<Vec<ThemeMeta>, String> {
    let db = state.0.lock().map_err(|e| e.to_string())?;
    let mut stmt = db
        .prepare("SELECT slug, name, swatch FROM themes ORDER BY rowid")
        .map_err(|e| e.to_string())?;
    let themes = stmt
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
        .unwrap_or_else(|_| "cantaloupe".to_string());

    let (name, css_vars_str): (String, String) = db
        .query_row(
            "SELECT name, css_vars FROM themes WHERE slug = ?1",
            params![slug],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| e.to_string())?;

    let css_vars: HashMap<String, String> =
        serde_json::from_str(&css_vars_str).map_err(|e| e.to_string())?;

    Ok(ThemeData {
        slug,
        name,
        css_vars,
    })
}

#[tauri::command]
pub fn set_theme(slug: String, state: tauri::State<DbState>) -> Result<(), String> {
    let db = state.0.lock().map_err(|e| e.to_string())?;
    db.execute(
        "INSERT OR REPLACE INTO user_preferences (id, current_theme) VALUES (1, ?1)",
        params![slug],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
