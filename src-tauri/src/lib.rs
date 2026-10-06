//! Tauri-schil: verbindt de UI met de radio-service.
//!
//! De UI roept deze commando's aan met `invoke(...)` en luistert naar het event
//! `radio-state` voor elke wijziging (ook als iemand aan de knop draait).

use std::sync::{Arc, Mutex};

use ft991a_radio::{Action, Radio, RadioState};
use ft991a_sim::SimRadio;
use ft991a_transport::serial::{self, PortInfo};
use serde::Deserialize;
use tauri::{AppHandle, Emitter, State};

/// Naam van het event met de nieuwe toestand.
const STATE_EVENT: &str = "radio-state";

#[derive(Default)]
struct AppState {
    radio: Mutex<Option<Arc<Radio>>>,
    /// Alleen gevuld als we met de simulator verbonden zijn.
    sim: Mutex<Option<SimRadio>>,
}

/// Waarmee verbinden: `{"kind": "simulator"}` of
/// `{"kind": "serial", "port": "COM5", "baud": 38400}`.
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Target {
    Simulator,
    Serial { port: String, baud: Option<u32> },
}

/// Fouten gaan als leesbare tekst naar de UI.
type CmdResult<T> = Result<T, String>;

#[tauri::command]
fn list_ports() -> Vec<PortInfo> {
    serial::list_ports()
}

#[tauri::command]
async fn connect(
    app: AppHandle,
    state: State<'_, AppState>,
    target: Target,
) -> CmdResult<RadioState> {
    disconnect_inner(&state);

    let (sim, (reader, writer)) = match target {
        Target::Simulator => {
            let sim = SimRadio::new();
            let rw = sim.connect();
            (Some(sim), rw)
        }
        Target::Serial { port, baud } => {
            let rw = serial::open(&port, baud.unwrap_or(serial::DEFAULT_BAUD))
                .map_err(|e| e.to_string())?;
            (None, rw)
        }
    };

    let emitter = app.clone();
    let listener = Box::new(move |s: &RadioState| {
        let _ = emitter.emit(STATE_EVENT, s);
    });
    // Verbinden wacht op antwoorden van de radio: niet op de UI-thread doen.
    let radio =
        tauri::async_runtime::spawn_blocking(move || Radio::connect(reader, writer, listener))
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?;

    let snapshot = radio.state();
    *state.radio.lock().unwrap() = Some(Arc::new(radio));
    *state.sim.lock().unwrap() = sim;
    Ok(snapshot)
}

#[tauri::command]
fn disconnect(state: State<'_, AppState>) {
    disconnect_inner(&state);
}

fn disconnect_inner(state: &AppState) {
    // Weggooien sluit de poort en stopt de leesthread.
    state.radio.lock().unwrap().take();
    state.sim.lock().unwrap().take();
}

#[tauri::command]
async fn radio_action(state: State<'_, AppState>, action: Action) -> CmdResult<RadioState> {
    let radio = state
        .radio
        .lock()
        .unwrap()
        .clone()
        .ok_or("niet verbonden met een radio")?;
    tauri::async_runtime::spawn_blocking(move || radio.apply(action))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}

/// Alleen voor de simulator: doet alsof er aan de VFO-knop gedraaid wordt.
#[tauri::command]
fn sim_turn_dial(state: State<'_, AppState>, delta_hz: i64) -> CmdResult<()> {
    let sim = state.sim.lock().unwrap();
    let sim = sim.as_ref().ok_or("alleen beschikbaar met de simulator")?;
    sim.turn_dial(delta_hz);
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            list_ports,
            connect,
            disconnect,
            radio_action,
            sim_turn_dial
        ])
        .run(tauri::generate_context!())
        .expect("de app kon niet starten");
}
