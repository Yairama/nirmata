use serde::Deserialize;
use std::collections::HashMap;
use tauri::{
    App, AppHandle, Emitter, Manager, State,
    menu::{MenuBuilder, MenuItem, MenuItemBuilder, PredefinedMenuItem, SubmenuBuilder},
};

pub const ACTION_IDS: &[&str] = &[
    "project.new",
    "project.open",
    "project.close",
    "app.quit",
    "edit.world",
    "edit.propose",
    "view.palette",
    "view.changes",
    "view.home",
    "view.world",
    "view.chronology",
    "view.assistant",
    "view.narrative",
    "view.simulation",
    "view.imports",
    "view.versions",
    "settings.open",
    "help.open",
    "help.onboarding",
    "help.about",
];

const WORLD_ACTIONS: &[&str] = &[
    "view.palette",
    "view.changes",
    "view.home",
    "view.world",
    "view.chronology",
    "view.assistant",
    "view.narrative",
    "view.simulation",
    "view.imports",
    "view.versions",
    "help.onboarding",
];

const WRITE_ACTIONS: &[&str] = &["edit.world", "edit.propose"];

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DesktopMenuState {
    world_open: bool,
    read_only: bool,
    ai_busy: bool,
}

pub struct DesktopMenuItems(HashMap<&'static str, MenuItem<tauri::Wry>>);

pub fn install(app: &mut App) -> tauri::Result<()> {
    let project_new = item(app, "project.new", "Nuevo mundo…", Some("CmdOrCtrl+N"))?;
    let project_open = item(app, "project.open", "Abrir mundo…", Some("CmdOrCtrl+O"))?;
    let project_close = item(
        app,
        "project.close",
        "Cerrar mundo",
        Some("CmdOrCtrl+Shift+W"),
    )?;
    let app_quit = item(app, "app.quit", "Salir de Nirmata", quit_accelerator())?;
    let project = SubmenuBuilder::new(app, "Proyecto")
        .item(&project_new)
        .item(&project_open)
        .separator()
        .item(&project_close)
        .item(&app_quit)
        .build()?;

    let cut = PredefinedMenuItem::cut(app, Some("Cortar"))?;
    let copy = PredefinedMenuItem::copy(app, Some("Copiar"))?;
    let paste = PredefinedMenuItem::paste(app, Some("Pegar"))?;
    let select_all = PredefinedMenuItem::select_all(app, Some("Seleccionar todo"))?;
    let edit_world = item(app, "edit.world", "Editar mundo y calendario…", None)?;
    let edit_propose = item(app, "edit.propose", "Proponer cambios…", None)?;
    let edit = SubmenuBuilder::new(app, "Editar")
        .item(&cut)
        .item(&copy)
        .item(&paste)
        .item(&select_all)
        .separator()
        .item(&edit_world)
        .item(&edit_propose)
        .build()?;

    let palette = item(
        app,
        "view.palette",
        "Buscar y ejecutar acciones…",
        Some("CmdOrCtrl+K"),
    )?;
    let changes = item(app, "view.changes", "Cambios pendientes", None)?;
    let home = item(app, "view.home", "Inicio", None)?;
    let world = item(app, "view.world", "Mundo", None)?;
    let chronology = item(app, "view.chronology", "Cronología", None)?;
    let assistant = item(app, "view.assistant", "Asistente", None)?;
    let narrative = item(app, "view.narrative", "Estudio narrativo", None)?;
    let simulation = item(app, "view.simulation", "Simulación", None)?;
    let imports = item(app, "view.imports", "Importaciones", None)?;
    let versions = item(app, "view.versions", "Versiones", None)?;
    let view = SubmenuBuilder::new(app, "Ver")
        .item(&palette)
        .item(&changes)
        .separator()
        .item(&home)
        .item(&world)
        .item(&chronology)
        .item(&assistant)
        .item(&narrative)
        .item(&simulation)
        .item(&imports)
        .item(&versions)
        .build()?;

    let settings_open = item(app, "settings.open", "Abrir Settings…", Some("CmdOrCtrl+,"))?;
    let settings = SubmenuBuilder::new(app, "Settings")
        .item(&settings_open)
        .build()?;

    let help_open = item(app, "help.open", "Centro de ayuda", Some("F1"))?;
    let onboarding = item(
        app,
        "help.onboarding",
        "Volver a mostrar primeros pasos",
        None,
    )?;
    let about = item(app, "help.about", "Acerca de Nirmata", None)?;
    let help = SubmenuBuilder::new(app, "Ayuda")
        .item(&help_open)
        .item(&onboarding)
        .separator()
        .item(&about)
        .build()?;

    let menu = MenuBuilder::new(app)
        .item(&project)
        .item(&edit)
        .item(&view)
        .item(&settings)
        .item(&help)
        .build()?;
    app.set_menu(menu)?;
    app.manage(DesktopMenuItems(HashMap::from([
        ("project.new", project_new),
        ("project.open", project_open),
        ("project.close", project_close),
        ("app.quit", app_quit),
        ("edit.world", edit_world),
        ("edit.propose", edit_propose),
        ("view.palette", palette),
        ("view.changes", changes),
        ("view.home", home),
        ("view.world", world),
        ("view.chronology", chronology),
        ("view.assistant", assistant),
        ("view.narrative", narrative),
        ("view.simulation", simulation),
        ("view.imports", imports),
        ("view.versions", versions),
        ("settings.open", settings_open),
        ("help.open", help_open),
        ("help.onboarding", onboarding),
        ("help.about", about),
    ])));
    app.on_menu_event(|handle, event| {
        let id = event.id().as_ref();
        if ACTION_IDS.contains(&id) {
            let _ = handle.emit("desktop-menu-action", id);
        }
    });
    Ok(())
}

#[tauri::command]
pub fn set_desktop_menu_state(
    items: State<'_, DesktopMenuItems>,
    input: DesktopMenuState,
) -> Result<(), String> {
    for id in ["project.new", "project.open"] {
        set_enabled(&items, id, enabled_for(id, input))?;
    }
    for id in WORLD_ACTIONS {
        set_enabled(&items, id, enabled_for(id, input))?;
    }
    set_enabled(&items, "project.close", enabled_for("project.close", input))?;
    for id in WRITE_ACTIONS {
        set_enabled(&items, id, enabled_for(id, input))?;
    }
    Ok(())
}

fn enabled_for(id: &str, state: DesktopMenuState) -> bool {
    if matches!(id, "project.new" | "project.open") {
        return !state.world_open && !state.ai_busy;
    }
    if id == "project.close" {
        return state.world_open && !state.ai_busy;
    }
    if WRITE_ACTIONS.contains(&id) {
        return state.world_open && !state.read_only && !state.ai_busy;
    }
    if WORLD_ACTIONS.contains(&id) {
        return state.world_open;
    }
    true
}

#[tauri::command]
pub fn exit_application(app: AppHandle) {
    app.exit(0);
}

fn item(
    app: &App,
    id: &'static str,
    text: &'static str,
    accelerator: Option<&'static str>,
) -> tauri::Result<tauri::menu::MenuItem<tauri::Wry>> {
    let mut builder = MenuItemBuilder::with_id(id, text);
    if let Some(accelerator) = accelerator {
        builder = builder.accelerator(accelerator);
    }
    builder.build(app)
}

fn set_enabled(items: &DesktopMenuItems, id: &str, enabled: bool) -> Result<(), String> {
    let item = items
        .0
        .get(id)
        .ok_or_else(|| format!("menu item not found: {id}"))?;
    item.set_enabled(enabled).map_err(|error| error.to_string())
}

#[cfg(target_os = "macos")]
fn quit_accelerator() -> Option<&'static str> {
    Some("Cmd+Q")
}

#[cfg(not(target_os = "macos"))]
fn quit_accelerator() -> Option<&'static str> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn action_ids_are_unique() {
        assert_eq!(
            ACTION_IDS.len(),
            ACTION_IDS.iter().copied().collect::<HashSet<_>>().len()
        );
    }

    #[test]
    fn closed_project_exposes_only_project_entry_actions() {
        let state = DesktopMenuState {
            world_open: false,
            read_only: false,
            ai_busy: false,
        };
        assert!(enabled_for("project.new", state));
        assert!(enabled_for("project.open", state));
        assert!(!enabled_for("project.close", state));
        assert!(!enabled_for("view.world", state));
        assert!(!enabled_for("edit.propose", state));
        assert!(enabled_for("settings.open", state));
    }

    #[test]
    fn read_only_and_busy_disable_writes_without_hiding_read_navigation() {
        let read_only = DesktopMenuState {
            world_open: true,
            read_only: true,
            ai_busy: false,
        };
        assert!(!enabled_for("edit.world", read_only));
        assert!(!enabled_for("edit.propose", read_only));
        assert!(enabled_for("view.versions", read_only));

        let busy = DesktopMenuState {
            world_open: true,
            read_only: false,
            ai_busy: true,
        };
        assert!(!enabled_for("project.close", busy));
        assert!(!enabled_for("edit.world", busy));
        assert!(enabled_for("view.narrative", busy));
    }
}
