use chrono::{DateTime, Datelike, Local, Timelike};
use nusb::transfer::Interrupt;
use tokio::io::AsyncWriteExt;
use nusb::io::EndpointWrite;

pub trait Command 
{
    fn get_hid_code() -> &'static u8;
    fn get_op_code() -> &'static u8;
    fn get_data(&self) -> &[u8; 62];
}

pub struct HandshakeCommand 
{
    data: [u8; 62]
}

impl HandshakeCommand 
{
    pub fn new() -> HandshakeCommand
    {
        let mut data = [0u8; 62];
        data[2] = 0x01;
        HandshakeCommand { data: data }
    }
}

impl Command for HandshakeCommand
{
    fn get_hid_code() -> &'static u8 { &0x11 }

    fn get_op_code() -> &'static u8 { &0x80 }

    fn get_data(&self) -> &[u8; 62] { &self.data }
}

pub struct TimestampCommand
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

impl TimestampCommand
{
    pub fn new(with_timestamp: bool) -> TimestampCommand
    {
        let mut data = [0u8; 62];
        
        if with_timestamp 
        { 
            let dt = Local::now();
            
            data[1] = 0x01;
            data[4..9].copy_from_slice(&format_date(&dt));
            data[9] = 0x01;
        }
        TimestampCommand { data: data }
    }
}

impl Command for TimestampCommand
{
    fn get_hid_code() -> &'static u8 { &0x11 }
    fn get_op_code() -> &'static u8 { &0x84 }
    fn get_data(&self) -> &[u8; 62] { &self.data }
}

pub struct KeepaliveCommand;

impl Command for KeepaliveCommand
{
    fn get_hid_code() -> &'static u8 { &0x11 }

    fn get_op_code() -> &'static u8 { &0x14 }

    fn get_data(&self) -> &[u8; 62] { &[0;62] }
}

#[allow(dead_code)]
pub struct VolumeCommand
{
    data: [u8; 62]
}

#[allow(dead_code)]
impl VolumeCommand
{
    pub fn new(volume: u8) -> VolumeCommand
    {
        let mut data = [0;62];
        data[2] = volume;
        VolumeCommand { data: data }
    }
}

impl Command for VolumeCommand
{
    fn get_hid_code() -> &'static u8 { &0x11 }
    fn get_op_code() -> &'static u8 { &0x83 }
    fn get_data(&self) -> &[u8; 62] { &self.data }
}

pub async fn send_command(buf: &[u8;64], writer: &mut EndpointWrite<Interrupt>)
{
    writer.write(buf).await.unwrap_or_else(|_| { println!("Send command failed."); 0 });
    writer.flush().await.unwrap_or_else(|_| println!("Send command failed."));
}

pub fn build_command<T>(command: T) -> [u8; 64]
    where T: Command
{
    let mut buf = [0u8; 64];
    buf[0] = *T::get_hid_code();
    buf[1] = *T::get_op_code();
    buf[2..].copy_from_slice(command.get_data());
    buf
}
