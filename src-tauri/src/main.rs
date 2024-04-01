#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::{command, Manager, AppHandle, Window, Builder, WindowEvent, generate_context, generate_handler};
use std::{thread::{self, sleep}, time::Duration};
use std::sync::atomic::{AtomicBool, Ordering};
use rdev::{listen, Event, EventType};
use sysinfo::System;

#[derive(Clone, serde::Serialize)]
struct Payload {
  message: String,
}

#[derive(Clone, serde::Serialize)]
struct Payload2 {
  args: Vec<String>,
  cwd: String,
}

#[command]
fn kill_roblox() -> bool {
    return match System::new_all().processes_by_name("RobloxPlayerBeta.exe").next() {
        Some(process) => process.kill(),
        _ => false
    };
}

#[command]
fn is_roblox_running() -> bool {
    return System::new_all().processes_by_name("RobloxPlayerBeta.exe").next().is_some();
}

#[command]
fn log(message: String) {
    println!("[FRONTEND] {}", message);
}

static KEY_EVENTS_INITIALIZED: AtomicBool = AtomicBool::new(false);

#[command]
fn init_key_events(window: Window) {
    if !KEY_EVENTS_INITIALIZED.load(Ordering::Relaxed) {
        KEY_EVENTS_INITIALIZED.store(true, Ordering::Relaxed);
        
        thread::spawn(move || {
            let callback = move | event: Event | {
                if let EventType::KeyRelease(key) = event.event_type {
                    window.emit("key-press", Payload { message: format!("{:?}", key) }).unwrap();
                }
            };

            listen(callback).unwrap();
        });
    }
}

#[command]
fn eval(app: AppHandle, name: &str, code: &str) -> bool {
    return match app.get_window(name) {
        Some(window) => match window.eval(code) {
            Ok(_) => true,
            Err(_) => false
        },
        None => false
    };
}

fn main() {
    Builder::default()
        .on_window_event(|e| {
            if let WindowEvent::Resized(_) = e.event() {
                sleep(Duration::from_millis(5));
            }
        })
        .plugin(tauri_plugin_single_instance::init(|app, argv, cwd| {
            app.emit_all("single-instance", Payload2 { args: argv, cwd }).unwrap();
        }))
        .invoke_handler(generate_handler![init_key_events, is_roblox_running, kill_roblox, eval, log])
        .run(generate_context!())
        .expect("Failed to launch application.");
}