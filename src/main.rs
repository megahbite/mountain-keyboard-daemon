use nusb::Endpoint;
use nusb::io::EndpointWrite;
use nusb::list_devices;
use nusb::transfer::{Interrupt, Out, In};
use tokio::sync::mpsc;
mod api;

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
        
        do_thing(endpoint_in, writer).await;
    }
}

async fn do_thing(mut reader: Endpoint<Interrupt, In>, mut writer: EndpointWrite<Interrupt>)
{
    let (tx, mut rx ) = mpsc::channel::<[u8; 64]>(10);

    // Read from 0x84 endpoint in a loop
    let _read_task = tokio::spawn(async move 
        {
            let buffer = reader.allocate(64);
            // Initial read
            reader.submit(buffer);
            loop 
            {
                let buffer = reader.allocate(64);
                reader.submit(buffer);
                if let Ok(result) = reader.next_complete().await.into_result()
                {
                    let mut to_send = [0u8; 64];
                    to_send.copy_from_slice(&result.into_vec());
                    tx.send(to_send).await.unwrap();
                }
            }
        }
    );

    let (tx_write, mut rx_write) = mpsc::channel::<[u8; 64]>(5);
    
    // Write to 0x05 endpoint from queue
    let _write_task = tokio::spawn(async move
        {
            loop 
            {
                if let Some(packet) = rx_write.recv().await
                {
                    println!("Sending {:x}", packet[1]);
                    api::command::send_packet(&packet, &mut writer).await;
                }
            }
        }
    );
    
    let tx_write2 = tx_write.clone();
    // Our keepalive task
    let _keepalive_task = tokio::spawn(async move 
        {
            loop {
                tx_write.send(api::command::build_packet(api::command::KeepalivePacket{})).await.unwrap();
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            }
        }
    );

    // State machine
    let _program_task = tokio::spawn( async move 
        {
            let mut state = State::NotReady;

            loop 
            {
                match state
                {
                    State::NotReady => 
                    {
                        if let Some(data) = rx.recv().await
                        {
                            if data[0] == 0x11
                            {
                                state = State::Handshake;
                                println!("Starting handshake");
                            }
                            else
                            {
                                println!("Nope");
                            }
                        }
                    },
                    State::Handshake => 
                    {
                        tx_write2.send(api::command::build_packet(api::command::HandshakePacket::new())).await.unwrap();
                        loop
                        {
                            if let Some(data) = rx.recv().await
                            {
                                if data[1] == 0x80
                                {
                                    println!("Got handshake ack!");
                                }
                            }
                        }
                    },
                    State::Handshook => {}
                }
            }
        }
    ).await;
}
