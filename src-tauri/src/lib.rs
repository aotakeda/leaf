mod dictionary;
mod epub_meta;
mod library;
mod wiktionary;

use dictionary::{Definition, Dictionary};
use library::{Book, Library};
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::ipc::{InvokeBody, Request, Response};
use tauri::{Manager, State};

struct AppState {
    library: Mutex<Library>,
    dictionary: Dictionary,
}

#[tauri::command]
fn list_books(state: State<AppState>) -> Vec<Book> {
    state.library.lock().unwrap().list()
}

#[tauri::command]
async fn import_books(state: State<'_, AppState>, paths: Vec<String>) -> Result<Vec<Book>, String> {
    let mut lib = state.library.lock().unwrap();
    let mut imported = Vec::new();
    let mut errors = Vec::new();
    for p in paths {
        match lib.import(&PathBuf::from(&p)) {
            Ok(book) => imported.push(book),
            Err(e) => errors.push(e),
        }
    }
    if imported.is_empty() && !errors.is_empty() {
        return Err(errors.join("\n"));
    }
    Ok(imported)
}

#[tauri::command]
async fn open_book(state: State<'_, AppState>, id: String) -> Result<Response, String> {
    let bytes = state.library.lock().unwrap().open(&id)?;
    Ok(Response::new(bytes))
}

#[tauri::command]
fn get_cover(state: State<AppState>, id: String) -> Result<Response, String> {
    let path = state.library.lock().unwrap().cover_path(&id);
    std::fs::read(path)
        .map(Response::new)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn save_cover(state: State<AppState>, request: Request) -> Result<(), String> {
    let id = request
        .headers()
        .get("book-id")
        .and_then(|v| v.to_str().ok())
        .ok_or("missing book-id")?
        .to_string();
    let InvokeBody::Raw(bytes) = request.body() else {
        return Err("expected raw bytes".into());
    };
    state.library.lock().unwrap().set_cover(&id, bytes)
}

#[tauri::command]
fn save_progress(
    state: State<AppState>,
    id: String,
    location: String,
    progress: f32,
) -> Result<(), String> {
    state
        .library
        .lock()
        .unwrap()
        .set_progress(&id, location, progress)
}

#[tauri::command]
fn update_meta(
    state: State<AppState>,
    id: String,
    title: Option<String>,
    author: Option<String>,
) -> Result<(), String> {
    state.library.lock().unwrap().set_meta(&id, title, author)
}

#[tauri::command]
fn get_locations(state: State<AppState>, id: String) -> Option<String> {
    let path = state.library.lock().unwrap().locations_path(&id);
    std::fs::read_to_string(path).ok()
}

#[tauri::command]
fn save_locations(state: State<AppState>, id: String, data: String) -> Result<(), String> {
    let path = state.library.lock().unwrap().locations_path(&id);
    std::fs::write(path, data).map_err(|e| e.to_string())
}

#[tauri::command]
fn remove_book(state: State<AppState>, id: String) -> Result<(), String> {
    state.library.lock().unwrap().remove(&id)
}

#[tauri::command]
async fn define(
    state: State<'_, AppState>,
    word: String,
    langs: Vec<String>,
) -> Result<Option<Definition>, String> {
    state.dictionary.define(&word, &langs).await
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let root = app.path().app_data_dir()?;
            app.manage(AppState {
                library: Mutex::new(Library::load(root)),
                dictionary: Dictionary::new(),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_books,
            import_books,
            open_book,
            get_cover,
            save_cover,
            save_progress,
            update_meta,
            get_locations,
            save_locations,
            remove_book,
            define
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
