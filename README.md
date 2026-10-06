# Yaesu FT-991A app

Een app om de Yaesu FT-991A via CAT (de USB-kabel) te bedienen en later volledig
te programmeren. Draait op Windows, Linux en macOS; later ook als server op een
Raspberry Pi naast de radio, zodat je met de browser op je telefoon kunt bedienen.

**Wat het nu kan (stap 3 van het plan: basisbediening)**

- Verbinden met de radio via de USB-poort, of met een **simulator** (nep-FT-991A) zonder radio.
- VFO-A en VFO-B: frequentie uitlezen, intypen (in MHz), omhoog/omlaag in stappen van 10 Hz tot 1 MHz, of scrollen op de cijfers.
- Mode kiezen: alle 14 modes (LSB, USB, CW, CW-R, AM, AM-N, FM, FM-N, DATA-…, RTTY-…, C4FM).
- Filter: bandbreedte (per mode de juiste tabel), narrow aan/uit en IF-shift (−1200 … +1200 Hz).
- Draai je aan de knop van de radio, dan volgt het scherm direct (Auto Information, `AI1;`).

## Opbouw

```
crates/
  cat/        CAT-protocol: commando's opbouwen en antwoorden lezen, met tests
  transport/  seriële poort, één vraag tegelijk, meldingen van de radio doorgeven
  radio/      radio-service: houdt de toestand bij en voert acties uit
  sim/        gesimuleerde FT-991A
src-tauri/    desktop-app (Tauri 2), verbindt het scherm met de radio-service
ui/           het scherm (Svelte + TypeScript)
```

De Rust-kern (`crates/`) weet niets van het scherm. Het scherm stuurt alleen
*acties* (bijv. `{"type": "set_mode", "mode": "USB"}`) en krijgt de *toestand*
terug. Daardoor kan dezelfde kern later zonder scherm op de Raspberry Pi draaien.

## Zelf draaien

Eenmalig installeren:

1. **Rust**: <https://rustup.rs>
2. **Node.js** 20 of nieuwer: <https://nodejs.org>
3. Alleen **Linux**: systeempakketten, bijv. op Debian/Ubuntu
   ```
   sudo apt install libwebkit2gtk-4.1-dev libudev-dev libayatana-appindicator3-dev librsvg2-dev build-essential
   ```
   en zet jezelf in de groep `dialout` om de seriële poort te mogen openen.
4. **Windows**: de Yaesu "Virtual COM Port Driver". **macOS**: de Silicon Labs CP210x-driver.

Daarna, in deze map:

```
npm install
npm run tauri dev       # app starten (eerste keer duurt het bouwen een paar minuten)
cargo test --workspace  # alle tests
npm run tauri build     # installatiebestand maken voor dit besturingssysteem
```

Kies in de app **Simulator** om zonder radio te proberen, of de COM-/USB-poort
van de radio. Zet op de radio menu **031 CAT RATE** op dezelfde baudrate als in
de app (standaard 38400).

## Nog te testen met de echte radio

- Welke van de twee USB-poorten CAT is (de app markeert de waarschijnlijke).
- 1 of 2 stopbits (nu 2) en RTS (staat hoog).
- Welke commando's de radio in FM met `?;` weigert; de simulator neemt aan: bandbreedte, narrow en IF-shift.
- Wat de radio met de bandbreedte-index doet bij wisselen van mode of narrow.
