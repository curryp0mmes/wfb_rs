use raptorq::{SourceBlockDecoder, EncodingPacket};
use std::mem::size_of;
use std::iter::once;
use std::collections::{HashMap, HashSet};

use crate::common::fec::{self, FecHeader, FEC_HEADER_SIZE};

pub(super) struct RXFec {
    magic: u32,
    fec_decoders: HashMap<u8, SourceBlockDecoder>,
    decoded_blocks: HashSet<u8>,
}

impl RXFec {
    pub fn new(magic: u32) -> Self {
        Self {
            magic,
            fec_decoders: HashMap::new(),
            decoded_blocks: HashSet::new(),
        }
    }
    pub fn process_fec_packet(
        &mut self,
        packet: &[u8],
    ) -> Option<Vec<Vec<u8>>> {

        // decoding fec header, returning the raw data if none is found
        let Some(fec_header) = FecHeader::from_bytes(self.magic, packet) else {
            return Some(vec![packet.to_vec()])
        };

        // dropping Fec Header part of the block id
        let packet = &packet[FEC_HEADER_SIZE..];

        // get block id:
        let block_id = packet.get(0)?;

        // Check if we've already successfully decoded this block
        if self.decoded_blocks.contains(block_id) {
            // Already decoded this block, ignore this packet
            return None;
        }

        // Get or create decoder for this block
        if !self.fec_decoders.contains_key(block_id) {
            // Create ObjectTransmissionInformation with proper parameters
            // (transfer_length, symbol_size, sub_symbol_size, source_symbols, repair_symbols)

            let (config, padding) = fec::get_raptorq_oti(fec_header.block_size, fec_header.packet_size);
            self.fec_decoders
                .insert(*block_id, SourceBlockDecoder::new(*block_id, &config, config.transfer_length() + padding));
        }

        let decoder = self.fec_decoders.get_mut(block_id).unwrap();
        
        let packet = EncodingPacket::deserialize(packet);

        // add packet to decoder
        // Try to decode with current packets
        if let Some(mut decoded_data) = decoder.decode(once(packet)) {
            // Successfully decoded! Get the original udp packages:
            let Some(num_pkgs_lim) = decoded_data.pop() else { return None };
            if decoded_data.len() < num_pkgs_lim as usize * size_of::<u16>() { return None };
            let indices_start_index = decoded_data.len() - num_pkgs_lim as usize * size_of::<u16>();
            let pkg_indices: Vec<_> = decoded_data[indices_start_index..]
                .chunks(size_of::<u16>())
                .map(|b| u16::from_le_bytes(b.try_into().unwrap()))
                .collect();
            let mut packets = Vec::new();
            for i in pkg_indices.windows(2) {
                let (start, end) = (i[0] as usize, i[1] as usize);
                packets.push(decoded_data[start..end].to_vec());
            }

            // Clean up
            self.fec_decoders.remove(block_id);
            self.decoded_blocks.insert(*block_id);

            return Some(packets);
        }

        // Clean up old decoders to prevent memory leak
        // Remove decoders older than current block_id - 64
        let cleanup_limit = 64;
        let cleanup_threshold_high = block_id.wrapping_add(cleanup_limit);
        let cleanup_threshold_low = block_id.wrapping_sub(cleanup_limit);
        let condition: Box<dyn Fn(u8) -> bool> = if cleanup_threshold_high > cleanup_threshold_low {
            Box::new(|a| cleanup_threshold_low < a && a < cleanup_threshold_high)
        } else {
            Box::new(|a| cleanup_threshold_low < a || a < cleanup_threshold_high)
        };
        self.fec_decoders.retain(|&k, _| condition(k));
        // Also clean up decoded blocks tracker
        self.decoded_blocks.retain(|&k| condition(k));

        return None; // Need more packets
    }
}

