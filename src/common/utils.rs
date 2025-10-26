use std::process::Command;

// This function sets up wifi drivers for sending data via monitor mode. It is designed for OpenIPC Cameras and requires iw
pub fn set_monitor_mode(interface_name: &str) -> Result<(), Box<dyn std::error::Error>> {
    if let Err(_) = Command::new("modprobe").arg("8812eu").output() {
        //Pass, driver must not be set
    }

    Command::new("ip")
        .args(["link", "set", interface_name, "down"])
        .output()?;
    Command::new("iw")
        .args(["dev", interface_name, "set", "monitor", "otherbss"])
        .output()
        .expect("Could not find iw command, can't set monitor mode");
    Command::new("ip")
        .args(["link", "set", interface_name, "up"])
        .output()?;
    Command::new("iw")
        .args(["dev", interface_name, "set", "channel", "149"])
        .output()?;
    Ok(())
}

pub fn set_tx_power(interface_name: &str, tx_power: u8) -> Result<(), Box<dyn std::error::Error>> {
    Command::new("iw")
        .args(["dev", interface_name, "set", "txpower", "fixed", format!("{}", tx_power as u16 * 50).as_str()])
        .output()?;
    Ok(())
}
