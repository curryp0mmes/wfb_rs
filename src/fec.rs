use std::mem::size_of;

use raptorq::ObjectTransmissionInformation;

// FEC Header constants and structures
pub const FEC_HEADER_SIZE: usize = size_of::<FecHeader>();

pub fn get_raptorq_oti(block_size: u16, wifi_packet_size: u16) -> ObjectTransmissionInformation {
    ObjectTransmissionInformation::with_defaults(block_size as u64, wifi_packet_size)
}

#[derive(Debug, Clone, Copy)]
pub struct FecHeader {
    pub magic: u32,         // 4 bytes - magic number to identify FEC packets
    pub block_id: u8,       // 1 byte - identifies which data block this belongs to
    pub block_size: u16,    // 2 bytes - the total size of the current fec block in bytes
}

impl FecHeader {
    pub const MAGIC: u32 = 0x46454332; // "FEC2" in ASCII

    pub fn new(block_id: u8, block_size: u16) -> Self {
        Self {
            magic: Self::MAGIC,
            block_id,
            block_size,
        }
    }

    pub fn to_bytes(&self) -> [u8; FEC_HEADER_SIZE] {
        let mut bytes = [0u8; FEC_HEADER_SIZE];
        bytes[0..4].copy_from_slice(&self.magic.to_le_bytes());
        bytes[4..5].copy_from_slice(&self.block_id.to_le_bytes());
        bytes[5..7].copy_from_slice(&self.block_size.to_le_bytes());
        bytes
    }

    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < FEC_HEADER_SIZE {
            return None;
        }

        let magic = u32::from_le_bytes(bytes[0..4].try_into().unwrap());
        if magic != Self::MAGIC {
            return None;
        }

        let block_id = bytes[4];
        let block_size = u16::from_le_bytes(bytes[5..7].try_into().unwrap());

        Some(Self {
            magic,
            block_id,
            block_size
        })
    }
}
