use nusb::transfer::Interrupt;
use tokio::io::AsyncWriteExt;
use nusb::io::EndpointWrite;

pub struct HandshakePacket 
{
    data: [u8; 62]
}

pub trait Packet 
{
    fn get_hid_code(&self) -> &'static u8;
    fn get_op_code(&self) -> &'static u8;
    fn get_data(&self) -> &[u8; 62];
}

impl HandshakePacket 
{
    pub fn new() -> HandshakePacket
    {
        let mut data = [0u8; 62];
        data[2] = 0x01;
        HandshakePacket { data: data }
    }
}

impl Packet for HandshakePacket
{
    fn get_hid_code(&self) -> &'static u8 { &0x11 }

    fn get_op_code(&self) -> &'static u8 { &0x80 }

    fn get_data(&self) -> &[u8; 62] { &self.data }
}

pub struct KeepalivePacket;

impl Packet for KeepalivePacket
{
    fn get_hid_code(&self) -> &'static u8 { &0x11 }

    fn get_op_code(&self) -> &'static u8 { &0x14 }

    fn get_data(&self) -> &[u8; 62] { &[0;62] }
}

pub async fn send_packet(buf: &[u8;64], writer: &mut EndpointWrite<Interrupt>)
{
    writer.write(buf).await.unwrap();
    writer.flush().await.unwrap();
}

pub fn build_packet<T>(packet: T) -> [u8; 64]
    where T: Packet
{
    let mut buf = [0u8; 64];
    buf[0] = *packet.get_hid_code();
    buf[1] = *packet.get_op_code();
    buf[2..].copy_from_slice(packet.get_data());
    buf
}
