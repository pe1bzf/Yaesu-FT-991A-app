// De enige plek waar de UI met de radio praat. Nu via Tauri; later komt hier
// een WebSocket-variant bij voor de Raspberry Pi, met dezelfde functies.

import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { Action, PortInfo, RadioState, Target } from "./types";

export const available = isTauri();

export function listPorts(): Promise<PortInfo[]> {
  return invoke("list_ports");
}

export function connect(target: Target): Promise<RadioState> {
  return invoke("connect", { target });
}

export function disconnect(): Promise<void> {
  return invoke("disconnect");
}

export function act(action: Action): Promise<RadioState> {
  return invoke("radio_action", { action });
}

export function simTurnDial(deltaHz: number): Promise<void> {
  return invoke("sim_turn_dial", { deltaHz });
}

export function onState(handler: (s: RadioState) => void): Promise<UnlistenFn> {
  return listen<RadioState>("radio-state", (e) => handler(e.payload));
}
