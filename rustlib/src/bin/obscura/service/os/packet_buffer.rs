const PACKET_CAPACITY: usize = 100;
const BUFFER_CAPACITY: usize = 1500 * 100 + u16::MAX as usize;
pub struct PacketBuffer {
    buffer: Box<[u8; BUFFER_CAPACITY]>,
    buffer_len: usize,
    lengths: Box<[u16; PACKET_CAPACITY]>,
    lengths_len: usize,
}

impl PacketBuffer {
    pub fn buffer(&mut self) -> Option<&mut [u8]> {
        let remainder = &mut self.buffer[self.buffer_len..];
        (self.lengths_len < self.lengths.len() && remainder.len() > usize::from(u16::MAX)).then_some(remainder)
    }
    pub fn commit(&mut self, size: u16) {
        self.buffer_len += usize::from(size);
        self.lengths[self.lengths_len] = size;
        self.lengths_len += 1;
    }
    pub fn take_iter(&mut self) -> PacketBufferIter<'_> {
        let iter = PacketBufferIter { buffer: &self.buffer[..self.buffer_len], lengths: &self.lengths[..self.lengths_len] };
        self.buffer_len = 0;
        self.lengths_len = 0;
        iter
    }
}

impl Default for PacketBuffer {
    fn default() -> Self {
        PacketBuffer {
            buffer: [0u8; BUFFER_CAPACITY].into(),
            buffer_len: 0,
            lengths: [0u16; PACKET_CAPACITY].into(),
            lengths_len: 0,
        }
    }
}

pub struct PacketBufferIter<'a> {
    buffer: &'a [u8],
    lengths: &'a [u16],
}

impl<'a> Iterator for PacketBufferIter<'a> {
    type Item = &'a [u8];

    fn next(&mut self) -> Option<Self::Item> {
        let packet_len = usize::from(*self.lengths.first()?);
        self.lengths = &self.lengths[1..];
        let packet;
        (packet, self.buffer) = self.buffer.split_at(packet_len);
        Some(packet)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.lengths.len(), Some(self.lengths.len()))
    }
}
