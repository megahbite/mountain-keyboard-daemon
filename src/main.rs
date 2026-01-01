use futures_lite::stream::StreamExt;
use nusb::hotplug::HotplugEvent;
use nusb::{DeviceId, DeviceInfo, Endpoint, watch_devices};
use nusb::io::EndpointWrite;
use nusb::list_devices;
use nusb::transfer::{Interrupt, Out, In};
use std::sync::Arc;
use tokio::sync::mpsc::{self, Sender, Receiver};
use tokio::sync::{Mutex, Notify};
use tokio::task::yield_now;

mod api;
use api::command::*;
use api::reply::*;

#[cfg(feature = "pulse")]
mod pulse;

const MOUNTAIN_VENDOR_ID: u16 = 0x3282;
const EVEREST_PRODUCT_ID: u16 = 0x0001;
const IN_ENDPOINT_ADDR: u8 = 0x84;
const OUT_ENDPOINT_ADDR: u8 = 0x05;

#[tokio::main]
async fn main() {
    let mut usb_watcher  = watch_devices().expect("Couldn't get USB hotplug watcher");
    let device_id: Option<DeviceId> = None;
    let device_id_mutex = Arc::new(Mutex::new(device_id));
    let device_id_list_mutex = Arc::clone(&device_id_mutex);
    
    let (connect_tx, mut connect_rx) = mpsc::channel::<DeviceInfo>(1);
    let connect_list_tx = connect_tx.clone();
    let disconnect = Arc::new(Notify::new());
    let disconnected = Arc::clone(&disconnect);

    let volume_read = Arc::new(Notify::new());
    let volume_read_requested = Arc::clone(&volume_read);
    let (volume_tx, volume_rx) = mpsc::channel::<u8>(1);
    let volume_rx_mut = Arc::new(Mutex::new(volume_rx));

    #[cfg(feature = "pulse")]
    {
        use std::thread;

        thread::spawn(move || 
        {
            let mut pulse = pulse::Pulse::new();
            if !pulse.ready { pulse.wait_until_ready(); }
            loop
            {
                use futures_lite::future::block_on;

                block_on(volume_read_requested.notified());
                volume_tx.blocking_send(pulse.get_volume()).unwrap();
            }
        });
    }
    
    tokio::spawn(async move
    {        
        let mut device_list = list_devices().await.expect("Couldn't list USB devices");
        if let Some(device_info) = device_list.find(|device| device.vendor_id() == MOUNTAIN_VENDOR_ID && device.product_id() == EVEREST_PRODUCT_ID)
        {
            let mut device_id = device_id_list_mutex.lock().await;
            *device_id = Some(device_info.id());
            connect_list_tx.send(device_info).await.unwrap();
        }
    });

    tokio::spawn(async move 
    {
        while let Some(event) = usb_watcher.next().await
        {
            match event 
            {
                HotplugEvent::Connected(device_info) => 
                {
                    if device_info.vendor_id() == MOUNTAIN_VENDOR_ID && device_info.product_id() == EVEREST_PRODUCT_ID
                    {
                        let mut device_id = device_id_mutex.lock().await;
                        *device_id = Some(device_info.id());
                        connect_tx.send(device_info).await.unwrap();
                    }
                }
                HotplugEvent::Disconnected(disconnected_device_id) => 
                {
                    let device_id = device_id_mutex.lock().await;
                    if device_id.is_some() && device_id.unwrap() == disconnected_device_id
                    {
                        disconnect.notify_one();
                    }
                }
            }
        }
    });

    loop 
    {
        let my_volume_request = Arc::clone(&volume_read);
        let my_disconnected = Arc::clone(&disconnected);
        let my_volume_rx_mut = Arc::clone(&volume_rx_mut);
        println!("Awaiting connection from keyboard");
        if let Some(device_info) = connect_rx.recv().await
        {
            println!("Connected {}", device_info.product_string().unwrap());
            tokio::spawn(keyboard_connected(device_info, my_disconnected, my_volume_request, my_volume_rx_mut)).await.unwrap();
        }
        yield_now().await;
    }
}

async fn keyboard_connected(device_info: DeviceInfo, disconnected: Arc<Notify>, volume_request: Arc<Notify>, volume_rx: Arc<Mutex<Receiver<u8>>>)
{
    let device = device_info.open().await.expect("Could not open device");
    let interface = device.detach_and_claim_interface(3u8).await.expect("Could not claim interface");
    let endpoint_out = interface.endpoint::<Interrupt, Out>(OUT_ENDPOINT_ADDR).expect("Couldn't get outbound endpoint");
    let endpoint_in = interface.endpoint::<Interrupt, In>(IN_ENDPOINT_ADDR).expect("Couldn't get inbound endpoint");

    let writer = endpoint_out.writer(4096);

    let (tx, rx ) = mpsc::channel::<CommandState>(10);
    // Read from 0x84 endpoint in a loop
    let read_handle = tokio::spawn(read_task(endpoint_in, tx));

    let (tx_write, rx_write) = mpsc::channel::<[u8; 64]>(5);
    
    // Write to 0x05 endpoint from queue
    let write_handle = tokio::spawn(write_task(writer, rx_write));
    
    let tx_write2 = tx_write.clone();
    // Our keepalive task
    let keepalive_handle = tokio::spawn(async move 
        {
            loop 
            {
                tx_write.send(build_command(KeepaliveCommand{})).await.unwrap_or_else(|_| println!("Keepalive send failed"));
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                yield_now().await;
            }
        }
    );

    let program_handle = tokio::spawn(run_program(rx, tx_write2, volume_request, volume_rx));

    disconnected.notified().await;

    read_handle.abort();
    write_handle.abort();
    keepalive_handle.abort();
    program_handle.abort();

    println!("Keyboard disconnected");
}

async fn read_task(mut endpoint_in: Endpoint<Interrupt, In>, tx: Sender<CommandState>)
{
    let mut initialised = false;
    let mut last_display_state = DisplayState::Unknown;
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

            let command_state = handle_reply(reply, &mut initialised, &mut last_display_state);

            if command_state != CommandState::Idle
            {
                tx.send(command_state).await.unwrap();
            }
        }
        yield_now().await;
    }
}

async fn write_task(mut writer: EndpointWrite<Interrupt>, mut rx: Receiver<[u8;64]>)
{
    loop 
    {
        if let Some(packet) = rx.recv().await
        {
            send_command(&packet, &mut writer).await;
        }
        yield_now().await;
    }
}

async fn run_program(mut new_state: Receiver<CommandState>, transmit: Sender<[u8;64]>, volume_request: Arc<Notify>, volume_rx: Arc<Mutex<Receiver<u8>>>)
{
    loop 
    {
        if let Some(state) = new_state.recv().await
        {
            match state
            {
                CommandState::SendHandshake => { transmit.send(build_command(HandshakeCommand::new())).await.unwrap(); }
                CommandState::CheckTimeUpdate => { transmit.send(build_command(TimestampCommand::new(false))).await.unwrap(); }
                CommandState::SendTimeUpdate => { transmit.send(build_command(TimestampCommand::new(true))).await.unwrap(); }
                CommandState::SendVolume => 
                {
                    #[cfg(feature = "pulse")]
                    {
                        volume_request.notify_one();
                        if let Some(volume) = volume_rx.lock().await.recv().await
                        {
                            transmit.send(build_command(VolumeCommand::new(volume))).await.unwrap();
                        }
                    }
                }
                _ => {}
            }
        }
        yield_now().await;
    }
}