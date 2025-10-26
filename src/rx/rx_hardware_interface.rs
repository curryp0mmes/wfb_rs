use pcap::{self, Active, Capture};
use radiotap::Radiotap;

use crate::common::hw_headers;

pub(super) struct RXHwInt {
    wifi_capture: Capture<Active>,
}


impl RXHwInt {
    pub fn new(wifi_device: String, channel_id: u32) -> Result<Self, Box<dyn std::error::Error>> {
        let wifi_capture = Self::open_wifi_capture(wifi_device, channel_id)?;
        Ok(Self { wifi_capture })
    }
    pub fn receive_packet(&mut self) -> Result<Option<Vec<u8>>, Box<dyn std::error::Error>> {
        match self.wifi_capture.next_packet() {
            Ok(packet) if packet.len() > 0 => {
                Ok(Self::process_packet(&packet)?)
            }
            Ok(_packet) => {
                //TODO reset fec (?)
                eprintln!("packet len <= 0");
                Ok(None)
            }
            Err(pcap::Error::TimeoutExpired) => {
                // Timeout is normal, continue
                Ok(None)
            }
            Err(e) => {
                eprintln!("Error receiving packet: {}", e);
                Ok(None)
            }
        }
    }
    // Reads and removes the radiotap and wifi headers
    pub fn process_packet(
        packet: &[u8],
    ) -> Result<Option<Vec<u8>>, Box<dyn std::error::Error>> {

        if packet.len() < 4 {
            eprintln!("packet too short");
            return Ok(None); // Too short for radiotap header
        }

        // Parse minimal radiotap header to get length
        let radiotap_len = u16::from_le_bytes([packet[2], packet[3]]) as usize;

        //Parse the whole radiotap header via library
        let _radiotap_header = Radiotap::from_bytes(packet)?;

        //println!("Received header: {:?}", radiotap_header);
        //let header_len = radiotap_header.header.size;

        // Skip radiotap header and IEEE 802.11 header
        let payload_start = radiotap_len + hw_headers::IEEE80211_HEADER.len();

        if packet.len() <= payload_start {
            eprintln!("packet has no payload");
            return Ok(None); // No payload
        }

        let payload = &packet[payload_start..];

        //there are four bytes at the end where i dont know where it is coming from, so i remove them
        //TODO figure out what that is
        let payload = &payload[..payload.len().saturating_sub(4)];

        Ok(Some(payload.to_vec()))
    }

    pub fn open_wifi_capture(wifi_device: String, channel_id: u32) -> Result<Capture<Active>, Box<dyn std::error::Error>> {
        let wifi_max_size = 4096;

        let wifi_card = pcap::Device::list()?
            .into_iter()
            .find(|dev| dev.name == wifi_device)
            .ok_or_else(|| format!("WiFi device {} not found", wifi_device))?;

        let mut cap = pcap::Capture::from_device(wifi_card)?
            .snaplen(wifi_max_size)
            .promisc(true)
            .timeout(10) // 10ms timeout
            .immediate_mode(true)
            .open()?;

        if cap.get_datalink() != pcap::Linktype::IEEE802_11_RADIOTAP {
            return Err(format!("Unknown encapsulation on interface {}", wifi_device).into());
        }

        // Set the BPF filter to match the original C++ code
        let filter = format!(
            "ether[0x0a:2]==0x5742 && ether[0x0c:4] == {:#010x}",
            channel_id
        );
        cap.filter(&filter, true)?;

        cap = cap.setnonblock()?;

        Ok(cap)
    }
}
