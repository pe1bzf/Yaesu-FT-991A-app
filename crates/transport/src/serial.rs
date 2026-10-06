//! Echte seriële poort (USB-kabel naar de radio).

use std::time::Duration;

use serde::Serialize;
use serialport::{DataBits, FlowControl, Parity, SerialPortType, StopBits};

use crate::{Reader, TransportError, Writer};

/// USB-chip in de FT-991A: Silicon Labs CP2105 **(niet geverifieerd)**.
const CP2105_VID: u16 = 0x10C4;
const CP2105_PID: u16 = 0xEA70;

/// Baudrate die de app standaard gebruikt; menu 031 CAT RATE moet hetzelfde staan.
pub const DEFAULT_BAUD: u32 = 38_400;

#[derive(Debug, Clone, Serialize)]
pub struct PortInfo {
    /// Naam om mee te openen, bijv. `COM5` of `/dev/ttyUSB0`.
    pub name: String,
    /// Leesbare omschrijving voor in de keuzelijst.
    pub description: String,
    /// Waarschijnlijk de CAT-poort van een FT-991A (USB-ID en interface 0).
    pub likely_cat: bool,
}

/// Alle seriële poorten op deze computer, de waarschijnlijke CAT-poort eerst.
pub fn list_ports() -> Vec<PortInfo> {
    let mut ports: Vec<PortInfo> = serialport::available_ports()
        .unwrap_or_default()
        .into_iter()
        // macOS: altijd de `cu.`-variant gebruiken, niet `tty.`.
        .filter(|p| !p.port_name.starts_with("/dev/tty."))
        .map(|p| {
            let (description, likely_cat) = match &p.port_type {
                SerialPortType::UsbPort(usb) => {
                    let is_yaesu = usb.vid == CP2105_VID && usb.pid == CP2105_PID;
                    let first_interface = usb.interface.is_none_or(|i| i == 0);
                    let product = usb.product.clone().unwrap_or_else(|| "USB".into());
                    (product, is_yaesu && first_interface)
                }
                SerialPortType::PciPort => ("PCI".into(), false),
                SerialPortType::BluetoothPort => ("Bluetooth".into(), false),
                SerialPortType::Unknown => (String::new(), false),
            };
            PortInfo {
                name: p.port_name,
                description,
                likely_cat,
            }
        })
        .collect();
    ports.sort_by_key(|p| (!p.likely_cat, p.name.clone()));
    ports
}

/// Opent de CAT-poort: 8 databits, geen pariteit, 2 stopbits, RTS hoog.
pub fn open(path: &str, baud: u32) -> Result<(Reader, Writer), TransportError> {
    let mut port = serialport::new(path, baud)
        .data_bits(DataBits::Eight)
        .parity(Parity::None)
        // Stopbits niet geverifieerd; Hamlib gebruikt 2.
        .stop_bits(StopBits::Two)
        .flow_control(FlowControl::None)
        // Korte lees-time-out zodat de leesthread regelmatig kan stoppen.
        .timeout(Duration::from_millis(100))
        .open()
        .map_err(|e| TransportError::Io(open_hint(path, &e)))?;
    // Met menu 033 CAT RTS = enable reageert de radio alleen als RTS hoog staat.
    port.write_request_to_send(true)
        .map_err(|e| TransportError::Io(e.to_string()))?;
    let reader = port
        .try_clone()
        .map_err(|e| TransportError::Io(e.to_string()))?;
    Ok((Box::new(reader), Box::new(port)))
}

fn open_hint(path: &str, e: &serialport::Error) -> String {
    let mut msg = format!("{path} openen mislukt: {e}");
    if cfg!(target_os = "linux") && e.to_string().to_lowercase().contains("permission") {
        msg.push_str(
            ". Zet je gebruiker in de groep 'dialout' (Debian/Ubuntu/Raspberry Pi OS) \
             of 'uucp' (Arch) en log opnieuw in.",
        );
    }
    msg
}
