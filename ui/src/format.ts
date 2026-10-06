export const FREQ_MIN_HZ = 30_000;
export const FREQ_MAX_HZ = 470_000_000;

/** 14250000 → "14.250.000" (MHz.kHz.Hz, zoals op de radio). */
export function formatFreq(hz: number): string {
  const s = String(hz).padStart(7, "0");
  const mhz = s.slice(0, -6).replace(/^0+(?=\d)/, "");
  return `${mhz}.${s.slice(-6, -3)}.${s.slice(-3)}`;
}

/**
 * Leest een ingetypte frequentie in MHz: "14.25", "14,250" of "438.650.000".
 * Geeft Hz terug, of null als het geen geldige frequentie is.
 */
export function parseFreq(input: string): number | null {
  let text = input.trim().replace(/\s/g, "").replace(/,/g, ".");
  if (!/^\d+(\.\d+)*$/.test(text)) return null;
  const parts = text.split(".");
  // "438.650.000": alles na het eerste punt is het deel onder de MHz.
  if (parts.length > 2) text = `${parts[0]}.${parts.slice(1).join("")}`;
  const hz = Math.round(parseFloat(text) * 1_000_000);
  return hz >= FREQ_MIN_HZ && hz <= FREQ_MAX_HZ ? hz : null;
}

export function formatHz(hz: number): string {
  return hz >= 1000 && hz % 100 === 0 ? `${hz / 1000} kHz` : `${hz} Hz`;
}
