// Dezelfde vormen als in de Rust-kern (crates/radio en crates/cat).

export type Vfo = "A" | "B";

export const MODES = [
  "LSB", "USB", "CW", "CW-R", "AM", "AM-N", "FM", "FM-N",
  "DATA-LSB", "DATA-USB", "DATA-FM", "RTTY-LSB", "RTTY-USB", "C4FM",
] as const;
export type Mode = (typeof MODES)[number];

export interface WidthOption {
  index: number;
  hz: number;
}

export interface FilterState {
  width_index: number;
  width_hz: number | null;
  narrow: boolean;
  if_shift_hz: number;
  width_options: WidthOption[];
}

export interface RadioState {
  connected: boolean;
  freq_a: number;
  freq_b: number;
  mode: Mode | null;
  filter: FilterState | null;
}

export type Action =
  | { type: "set_frequency"; vfo: Vfo; hz: number }
  | { type: "step_frequency"; vfo: Vfo; delta_hz: number }
  | { type: "set_mode"; mode: Mode }
  | { type: "set_width"; index: number }
  | { type: "set_narrow"; on: boolean }
  | { type: "set_if_shift"; hz: number }
  | { type: "swap_vfo" }
  | { type: "copy_a_to_b" }
  | { type: "copy_b_to_a" }
  | { type: "refresh" };

export interface PortInfo {
  name: string;
  description: string;
  likely_cat: boolean;
}

export type Target =
  | { kind: "simulator" }
  | { kind: "serial"; port: string; baud: number };
