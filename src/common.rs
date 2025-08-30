use clap::ValueEnum;

const MCS_KNOWN: u8 = 0x2 | 0x1 | 0x4 | 0x20 | 0x10; // Known MCS, 0x00 for 20MHz, 0x01 for 40MHz, etc.

static RADIOTAP_HEADER_HT: [u8; 13] = [
    0x00, 0x00, // <-- radiotap version
    0x0d, 0x00, // <- radiotap header length
    0x00, 0x80, 0x08, 0x00, // <-- radiotap present flags:  RADIOTAP_TX_FLAGS + RADIOTAP_MCS
    0x08, 0x00, // RADIOTAP_F_TX_NOACK
    MCS_KNOWN, 0x00, 0x00, // bitmap, flags, mcs_index
];

static RADIOTAP_HEADER_VHT: [u8; 22] = [
    0x00, 0x00, // <-- radiotap version
    0x16, 0x00, // <- radiotap header length
    0x00, 0x80, 0x20, 0x00, // <-- radiotap present flags: RADIOTAP_TX_FLAGS + VHT Information
    0x08, 0x00, // RADIOTAP_F_TX_NOACK
    0x45, 0x00, // Known VHT information: 0000 0000 0100 0101, BW, GI, STBC
    0x00, // Flags, BIT(0)=STBC, BIT(2)=GI
    0x04, // BW, 0:20M, 1:40M, 4:80, 11:160
    0x00, 0x00, 0x00, 0x00, // MCS_NSS[0:3]
    0x00, // Coding[3:0], BCC/LDPC
    0x00, // Group ID, not used
    0x00, 0x00, // Partial AID, not used
];

pub static IEEE80211_HEADER: [u8; 24] = [
    0x08, 0x01, 0x00,
    0x00, // data frame, not protected, from STA to DS via an AP, duration not set
    0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, // receiver is broadcast
    0x57, 0x42, 0xaa, 0xbb, 0xcc, 0xdd, // last four bytes will be replaced by channel_id
    0x57, 0x42, 0xaa, 0xbb, 0xcc, 0xdd, // last four bytes will be replaced by channel_id
    0x00, 0x00, // (seq_num << 4) + fragment_num
];

pub fn get_ieee80211_header(frame_type: u8, channel_id: u32, ieee_seq: u16) -> [u8; 24] {
    // Create IEEE 802.11 header (simplified)
    let mut ieee_header: [u8; 24] = IEEE80211_HEADER; // Basic 802.11 header size
    ieee_header[0] = frame_type; // Data frame

    ieee_header[12..16].copy_from_slice(&channel_id.to_be_bytes());
    ieee_header[18..22].copy_from_slice(&channel_id.to_be_bytes());

    // Set sequence number
    ieee_header[22] = (ieee_seq & 0xff) as u8;
    ieee_header[23] = ((ieee_seq >> 8) & 0xff) as u8;
    ieee_header
}

pub fn get_radiotap_headers(
    stbc: u8,
    ldpc: bool,
    short_gi: bool,
    bandwidth: Bandwidth,
    mcs_index: u8,
    vht_mode: bool,
    vht_nss: u8,
) -> Vec<u8> {
    let mut header = vec![];

    if !vht_mode {
        let mut flags = 0u8;
        match bandwidth {
            Bandwidth::Bw10 | Bandwidth::Bw20 => flags |= 0x0,
            Bandwidth::Bw40 => flags |= 0x1,
            _ => panic!("Invalid HT bandwidth"),
        }

        if short_gi {
            flags |= 0x4;
        }

        match stbc {
            0 => (),
            1 => flags |= 0x1 << 5,
            2 => flags |= 0x2 << 5,
            3 => flags |= 0x3 << 5,
            _ => panic!("Invalid HT STBC value"),
        }

        if ldpc {
            flags |= 0x10;
        }

        header.extend_from_slice(&RADIOTAP_HEADER_HT);
        header[11] = flags;
        header[12] = mcs_index;
    } else {
        let mut flags: u8 = 0;

        header.extend_from_slice(&RADIOTAP_HEADER_VHT);

        if short_gi {
            flags |= 0x4;
        }

        if stbc != 0 {
            flags |= 0x1;
        }

        header[13] = match bandwidth {
            Bandwidth::Bw10 | Bandwidth::Bw20 => 0x0,
            Bandwidth::Bw40 => 0x1,
            Bandwidth::Bw80 => 0x4,
            Bandwidth::Bw160 => 0xB,
        };

        if ldpc {
            header[18] = 0x1;
        }

        header[12] = flags;
        header[14] |= (mcs_index << 4) & 0xF0;
        header[14] |= vht_nss & 0xF;
    }
    header
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum Bandwidth {
    Bw10,
    Bw20,
    Bw40,
    Bw80,
    Bw160,
}

impl ToString for Bandwidth {
    fn to_string(&self) -> String {
        match self {
            Bandwidth::Bw10 => "10".to_string(),
            Bandwidth::Bw20 => "20".to_string(),
            Bandwidth::Bw40 => "40".to_string(),
            Bandwidth::Bw80 => "80".to_string(),
            Bandwidth::Bw160 => "160".to_string(),
        }
    }
}