#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use colored::{control, ColoredString, Colorize};
use lazy_static::lazy_static;
use rdev::{listen, Event, EventType};
use serde::Serialize;
use std::sync::{Arc, Mutex};
use std::{thread::{self, sleep}, time::Duration,};
use sysinfo::System;
use tauri::{command, generate_context, generate_handler, Builder, CustomMenuItem, Manager, SystemTray, SystemTrayEvent, SystemTrayMenu, Window, WindowEvent};
use tokio::fs;

#[derive(Clone, Serialize)]
struct Payload {
    message: String,
}

#[derive(Clone, Serialize)]
struct PayloadUpdate {
    message: bool,
}

#[derive(Clone, Serialize)]
struct Payload2 {
    args: Vec<String>,
    cwd: String,
}

struct Server {
    window: Window,
}

impl ws::Handler for Server {
    fn on_message(&mut self, data: ws::Message) -> ws::Result<()> {
        let mut connected = CONNECTED.lock().unwrap();

        if *connected {
            return Ok(());
        }

        let data_string = data.to_string();
        let mut parts = data_string.split(",");
        let type_value = match parts.next() {
            Some(val) => val.trim(),
            None => return Ok(()),
        };

        if type_value == "connect" {
            self.window
                .emit("update", PayloadUpdate { message: true })
                .unwrap();
            *connected = true;
        }

        return Ok(());
    }

    fn on_close(&mut self, _code: ws::CloseCode, _reason: &str) {
        let mut connected = CONNECTED.lock().unwrap();

        if *connected {
            self.window
                .emit("update", PayloadUpdate { message: false })
                .unwrap();
            *connected = false;
        }
    }
}

#[command]
fn kill_roblox() -> bool {
    return match System::new_all()
        .processes_by_name("RobloxPlayerBeta.exe")
        .next()
    {
        Some(process) => process.kill(),
        _ => false,
    };
}

#[command]
fn is_roblox_running() -> bool {
    return System::new_all()
        .processes_by_name("RobloxPlayerBeta.exe")
        .next()
        .is_some();
}

#[command]
async fn create_directory(path: String) -> bool {
    fs::create_dir_all(&path).await.is_ok()
}

#[command]
async fn write_file(path: String, data: String) -> bool {
    fs::write(&path, &data).await.is_ok()
}

#[command]
async fn write_binary_file(path: String, data: Vec<u8>) -> bool {
    fs::write(&path, &data).await.is_ok()
}

#[command]
async fn delete_directory(path: String) -> bool {
    fs::remove_dir_all(&path).await.is_ok()
}

#[command]
async fn delete_file(path: String) -> bool {
    fs::remove_file(&path).await.is_ok()
}

lazy_static! {
    static ref CONNECTED: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));
    static ref KEY_EVENTS_INITIALIZED: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));
    static ref WEBSOCKET_INITIALIZED: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));
    static ref WEBSOCKET: Arc<Mutex<Option<ws::Sender>>> = Arc::new(Mutex::new(None));
}

#[command]
fn init_key_events(window: Window) {
    let mut key_events_initialized = KEY_EVENTS_INITIALIZED.lock().unwrap();

    if *key_events_initialized {
        return;
    }

    *key_events_initialized = true;
    thread::spawn(move || {
        let callback = move |event: Event| {
            if let EventType::KeyRelease(key) = event.event_type {
                window
                    .emit(
                        "key-press",
                        Payload {
                            message: format!("{:?}", key),
                        },
                    )
                    .unwrap();
            }
        };

        listen(callback).unwrap();
    });
}

#[command]
fn init_websocket(window: Window, port: u16) {
    let mut websocket_initialized = WEBSOCKET_INITIALIZED.lock().unwrap();

    if *websocket_initialized {
        return;
    }

    *websocket_initialized = true;

    thread::spawn(move || {
        ws::listen(format!("127.0.0.1:{}", port), move |out: ws::Sender| {
            let cloned_window = window.clone();
            *WEBSOCKET.lock().unwrap() = Some(out.clone());
            return Server {
                window: cloned_window,
            };
        })
        .ok();
    });

    thread::spawn(move || loop {
        if let Some(websocket) = WEBSOCKET.lock().unwrap().clone() {
            websocket.send("kr-ping").unwrap();
        }

        sleep(Duration::from_millis(250));
    });
}

#[command]
fn execute_script(text: &str) {
    if let Some(websocket) = WEBSOCKET.lock().unwrap().clone() {
        websocket.send(text).unwrap();
    }
}

#[command]
fn log(message: String, _type: Option<String>) {
    let prefix: Option<ColoredString> = match _type {
        Some(_type) => match _type.as_str() {
            "info" => Some("[ INFO ]".cyan()),
            "success" => Some("[  OK  ]".green()),
            "warn" => Some("[ WARN ]".yellow()),
            "error" => Some("[ FAIL ]".red()),
            _ => None,
        },
        None => None,
    };

    if let Some(prefix) = prefix {
        println!("{} {}", prefix, message);
    } else {
        println!("{}", message);
    }
}

fn main() {
    control::set_virtual_terminal(true).ok();

    let toggle = CustomMenuItem::new("toggle".to_string(), "Toggle");
    let quit = CustomMenuItem::new("quit".to_string(), "Quit");
    let tray = SystemTrayMenu::new().add_item(toggle).add_item(quit);

    Builder::default()
        .on_window_event(|e| {
            if let WindowEvent::Resized(_) = e.event() {
                sleep(Duration::from_millis(5));
            }
        })
        .system_tray(SystemTray::new().with_menu(tray))
        .on_system_tray_event(|app, e| match e {
            SystemTrayEvent::MenuItemClick { id, .. } => {
                let window = app.get_window("main").unwrap();

                match id.as_str() {
                    "toggle" => window
                        .emit(
                            "toggle",
                            Payload {
                                message: "".to_string(),
                            },
                        )
                        .unwrap(),
                    "quit" => window
                        .emit(
                            "exit",
                            Payload {
                                message: "".to_string(),
                            },
                        )
                        .unwrap(),
                    _ => {}
                }
            }
            _ => {}
        })
        .plugin(tauri_plugin_single_instance::init(|app, argv, cwd| {
            app.emit_all("single-instance", Payload2 { args: argv, cwd })
                .unwrap();
        }))
        .invoke_handler(generate_handler![
            init_websocket,
            init_key_events,
            execute_script,
            is_roblox_running,
            kill_roblox,
            log,
            create_directory,
            write_file,
            write_binary_file,
            delete_directory,
            delete_file
        ])
        .run(generate_context!())
        .expect("Failed to launch application.");
}
