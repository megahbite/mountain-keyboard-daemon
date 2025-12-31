use chrono::{DateTime, Datelike, Local, Timelike};
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

pub struct TimestampPacket
{
    data: [u8; 62]
}

fn format_date(dt: &DateTime<Local>) -> [u8; 5]
{
    let month = dt.month();
    let month: u8 = month.try_into().unwrap_or(0);

    let day = dt.day();
    let day: u8 = day.try_into().unwrap_or(0);

    let hour = dt.hour();
    let hour: u8 = hour.try_into().unwrap_or(0);

    let min = dt.minute();
    let min: u8 = min.try_into().unwrap_or(0);

    let sec = dt.second();
    let sec: u8 = sec.try_into().unwrap_or(0);

    [month, day, hour, min, sec]
}

impl TimestampPacket
{
    pub fn new(with_timestamp: bool) -> TimestampPacket
    {
        let mut data = [0u8; 62];
        
        if with_timestamp 
        { 
            let dt = Local::now();
            
            data[1] = 0x01;
            data[4..9].copy_from_slice(&format_date(&dt));
            data[9] = 0x01;
        }
        TimestampPacket { data: data }
    }
}

impl Packet for TimestampPacket
{
    fn get_hid_code(&self) -> &'static u8 { &0x11 }
    fn get_op_code(&self) -> &'static u8 { &0x84 }
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
