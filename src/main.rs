use nusb::Endpoint;
use nusb::io::EndpointWrite;
use nusb::list_devices;
use nusb::transfer::{Interrupt, Out, In};
use tokio::sync::mpsc::{self, Sender, Receiver};

mod api;
use api::reply::ReplyType;

use crate::api::reply::TimeUpdateReply;

const MOUNTAIN_VENDOR_ID: u16 = 0x3282;
const EVEREST_PRODUCT_ID: u16 = 0x0001;
const IN_ENDPOINT_ADDR: u8 = 0x84;
const OUT_ENDPOINT_ADDR: u8 = 0x05;

#[derive(PartialEq, Clone)]
enum State 
{
    Initial,
    Idle,
    SendHandshake,
    CheckTimeUpdate,
    SendTimeUpdate
}

#[tokio::main]
async fn main() {
    if let Ok(mut devices) = list_devices().await {
        let device_info = devices.find(|device| device.vendor_id() == MOUNTAIN_VENDOR_ID && device.product_id() == EVEREST_PRODUCT_ID).expect("Device not found");
        println!("{:?} {:?}", device_info.product_string().unwrap(), device_info.serial_number());
        let device = device_info.open().await.expect("Could not open device");
        let interface = device.detach_and_claim_interface(3u8).await.expect("Could not claim interface");
        let endpoint_out = interface.endpoint::<Interrupt, Out>(OUT_ENDPOINT_ADDR).expect("Couldn't get outbound endpoint");
        let endpoint_in = interface.endpoint::<Interrupt, In>(IN_ENDPOINT_ADDR).expect("Couldn't get inbound endpoint");

        let writer = endpoint_out.writer(4096);

        let (tx, rx ) = mpsc::channel::<State>(10);
        // Read from 0x84 endpoint in a loop
        let read_task = tokio::spawn(read_task(endpoint_in, tx));

        let (tx_write, rx_write) = mpsc::channel::<[u8; 64]>(5);
        
        // Write to 0x05 endpoint from queue
        let write_task = tokio::spawn(write_task(writer, rx_write));
        
        let tx_write2 = tx_write.clone();
        // Our keepalive task
        let keepalive_task = tokio::spawn(async move 
            {
                loop 
                {
                    tx_write.send(api::command::build_packet(api::command::KeepalivePacket{})).await.unwrap();
                    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                }
            }
        );

        // State machine
        let program_task = tokio::spawn(run_program(rx, tx_write2));

        tokio::spawn(async move 
        {
            tokio::signal::ctrl_c().await.unwrap();
            println!("Cleaning up...");
            read_task.abort();
            write_task.abort();
            keepalive_task.abort();
            program_task.abort();
        }).await.unwrap();
    }
}

async fn read_task(mut endpoint_in: Endpoint<Interrupt, In>, tx: Sender<State>)
{
    let mut state = State::Initial;
    let buffer = endpoint_in.allocate(64);
    // Initial read - weird quirk with keyboard is that there has to always be two open read interrupts for it to respond
    endpoint_in.submit(buffer);
    loop 
    {
        let buffer = endpoint_in.allocate(64);
        endpoint_in.submit(buffer);
        if let Ok(result) = endpoint_in.next_complete().await.into_result()
        {
            let mut to_send = [0u8; 64];
            to_send.copy_from_slice(&result.into_vec());
            let reply = api::reply::ReplyPacket::from_buf(&to_send);
            
            let new_state = match state 
            {
                State::Initial => 
                { 
                    if reply.reply_type == ReplyType::Keepalive { State::SendHandshake }
                    else { State::Initial }
                }
                State::SendHandshake =>
                {
                    if reply.reply_type == ReplyType::Handshake { State::CheckTimeUpdate }
                    else { State::SendHandshake }
                }
                State::CheckTimeUpdate =>
                {
                    if reply.reply_type == ReplyType::TimeUpdate && let Ok(time_update) = TimeUpdateReply::parse_reply(reply)
                    {
                        if time_update.needs_update { State::SendTimeUpdate }
                        else { State::Idle }
                    }
                    else { State::CheckTimeUpdate }
                }
                State::SendTimeUpdate =>
                {
                    if reply.reply_type == ReplyType::TimeUpdate && let Ok(time_update) = TimeUpdateReply::parse_reply(reply)
                    {
                        if time_update.update_ack { State::Idle }
                        else { State::CheckTimeUpdate }
                    }
                    else { State::SendTimeUpdate }
                }
                _ => { State::Idle }
            };

            if state != new_state
            {
                let transmit_state = new_state.clone();
                state = new_state;
                tx.send(transmit_state).await.unwrap();
            }
        }
    }
}

async fn write_task(mut writer: EndpointWrite<Interrupt>, mut rx: Receiver<[u8;64]>)
{
    loop 
    {
        if let Some(packet) = rx.recv().await
        {
            api::command::send_packet(&packet, &mut writer).await;
        }
    }
}

async fn run_program(mut new_state: Receiver<State>, transmit: Sender<[u8;64]>)
{
    loop 
    {
        if let Some(state) = new_state.recv().await
        {
            match state
            {
                State::SendHandshake => { transmit.send(api::command::build_packet(api::command::HandshakePacket::new())).await.unwrap(); }
                State::CheckTimeUpdate => { transmit.send(api::command::build_packet(api::command::TimestampPacket::new(false))).await.unwrap(); }
                State::SendTimeUpdate => { transmit.send(api::command::build_packet(api::command::TimestampPacket::new(true))).await.unwrap(); }
                _ => {}
            }
        }
    }
}