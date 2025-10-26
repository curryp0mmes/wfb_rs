use std::mem::size_of;

use raptorq::ObjectTransmissionInformation;

// FEC Header constants and structures
pub const FEC_HEADER_SIZE: usize = size_of::<FecHeader>();

pub fn get_raptorq_oti(block_size: u16, wifi_packet_size: u16) -> (ObjectTransmissionInformation, u64) {
    let config = ObjectTransmissionInformation::with_defaults(block_size as u64, wifi_packet_size);
    let padding = config.symbol_size() as u64 - config.transfer_length() % config.symbol_size() as u64;
    (config, padding)
}

#[derive(Debug, Clone, Copy)]
pub struct FecHeader {
    pub magic: u32,         // 4 bytes - magic number to identify FEC packets
    pub block_size: u16,    // 2 bytes - the total size of the current fec block in bytes
    pub packet_size: u16,    // 2 bytes - the size of the wifi packet in bytes
}

impl FecHeader {
    pub fn new(magic: u32, block_size: u16, packet_size: u16) -> Self {
        Self {
            magic,
            block_size,
            packet_size,
        }
    }

    pub fn to_bytes(&self) -> [u8; FEC_HEADER_SIZE] {
        let mut bytes = [0u8; FEC_HEADER_SIZE];
        bytes[0..4].copy_from_slice(&self.magic.to_le_bytes());
        bytes[4..6].copy_from_slice(&self.block_size.to_le_bytes());
        bytes[6..8].copy_from_slice(&self.packet_size.to_le_bytes());
        bytes
    }

    #[cfg(feature = "receiver")]
    pub fn from_bytes(magic: u32, bytes: &[u8]) -> Option<Self> {
        if bytes.len() < FEC_HEADER_SIZE {
            return None;
        }

        let dec_magic = u32::from_le_bytes(bytes[0..4].try_into().unwrap());
        if dec_magic != magic {
            return None;
        }

        let block_size = u16::from_le_bytes(bytes[4..6].try_into().unwrap());
        let packet_size = u16::from_le_bytes(bytes[6..8].try_into().unwrap());

        Some(Self {
            magic,
            block_size,
            packet_size
        })
    }
}
