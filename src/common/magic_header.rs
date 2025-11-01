const MAGIC_HEADER_SIZE: usize = size_of::<MagicHeader>();

#[derive(Debug, Clone, Copy)]
pub struct MagicHeader {
    pub magic: u32,         // 4 bytes - magic number to identify wfb packets
}

impl MagicHeader {
    pub fn new(magic: u32) -> Self {
        Self {
            magic,
        }
    }

    pub fn new_fec(magic: u32) -> Self {
        Self {
            magic: !magic,
        }
    }

    pub fn to_bytes(&self) -> [u8; MAGIC_HEADER_SIZE] {
        let mut bytes = [0u8; MAGIC_HEADER_SIZE];
        bytes[0..4].copy_from_slice(&self.magic.to_le_bytes());
        bytes
    }

    #[cfg(feature = "receiver")]
    pub fn from_bytes<'a>(&self, bytes: &'a[u8]) -> Option<(bool, &'a[u8])> {
        if bytes.len() < MAGIC_HEADER_SIZE {
            return None;
        }

        let dec_magic = u32::from_le_bytes(bytes[0..4].try_into().unwrap());

        if dec_magic == self.magic {
            Some((false, &bytes[MAGIC_HEADER_SIZE..]))
        }

        else if dec_magic == !self.magic {
            Some((true, &bytes[MAGIC_HEADER_SIZE..]))
        }

        else {
            None
        }
    }
}
