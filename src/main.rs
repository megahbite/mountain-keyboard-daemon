use nusb::Endpoint;
use nusb::io::EndpointWrite;
use nusb::list_devices;
use nusb::transfer::{Interrupt, Out, In};
use tokio::sync::mpsc::{self, Sender, Receiver};

mod api;
use api::reply::{ReplyPacket, ReplyType};

const MOUNTAIN_VENDOR_ID: u16 = 0x3282;
const EVEREST_PRODUCT_ID: u16 = 0x0001;
const IN_ENDPOINT_ADDR: u8 = 0x84;
const OUT_ENDPOINT_ADDR: u8 = 0x05;

#[allow(dead_code)]
enum State
{
    NotReady,
    Handshake,
    Handshook
}

enum HandshakeState
{
    Initial,
    WithoutTimestamp,
    WithTimestamp
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

        let (tx, rx ) = mpsc::channel::<ReplyPacket>(10);
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

async fn read_task(mut endpoint_in: Endpoint<Interrupt, In>, tx: Sender<ReplyPacket>)
{
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
            tx.send(reply).await.unwrap();
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

async fn run_program(mut reply: Receiver<ReplyPacket>, transmit: Sender<[u8;64]>)
{
    let mut state = State::NotReady;

    loop 
    {
        match state
        {
            State::NotReady => 
            {
                if let Some(reply) = reply.recv().await
                {
                    if reply.reply_type == ReplyType::Keepalive
                    {
                        state = State::Handshake;
                        println!("Starting handshake");
                    }
                }
            },
            State::Handshake => 
            {
                let mut handshake_state = HandshakeState::Initial;
                'handshake: loop 
                {
                    match handshake_state
                    {
                        HandshakeState::Initial => 
                        {
                            transmit.send(api::command::build_packet(api::command::HandshakePacket::new())).await.unwrap();
                            loop
                            {
                                if let Some(reply) = reply.recv().await
                                {
                                    if reply.reply_type == ReplyType::Handshake
                                    {
                                        println!("Got handshake ack!");
                                        handshake_state = HandshakeState::WithoutTimestamp;
                                        break;
                                    }
                                }
                            }
                        },
                        HandshakeState::WithoutTimestamp => 
                        {
                            transmit.send(api::command::build_packet(api::command::HandshakeTimestampPacket::new(false))).await.unwrap();
                            loop
                            {
                                if let Some(reply) = reply.recv().await
                                {
                                    if reply.reply_type == ReplyType::TimeUpdate
                                    {
                                        println!("Keyboard wants a time update");
                                        handshake_state = HandshakeState::WithTimestamp;
                                        break;
                                    }
                                }
                            }
                        },
                        HandshakeState::WithTimestamp => 
                        {
                            transmit.send(api::command::build_packet(api::command::HandshakeTimestampPacket::new(true))).await.unwrap();
                            loop
                            {
                                if let Some(reply) = reply.recv().await
                                {
                                    if reply.reply_type == ReplyType::TimeUpdate
                                    {
                                        println!("Time updated");
                                        state = State::Handshook;
                                        break 'handshake;
                                    }
                                }
                            }
                        }
                    }
                }
                
            },
            State::Handshook => 
            {
                loop
                {
                    if let Some(_data) = reply.recv().await
                    {
                    }
                }
            }
        }
    }
}