use std::ops::Div;
use std::sync::{Arc, Mutex};

use libpulse_binding::mainloop::standard::Mainloop;
use libpulse_binding::proplist::Proplist;
use libpulse_binding::context::{Context, FlagSet as ContextFlagSet};
use libpulse_binding::volume::Volume;

pub struct Pulse
{
    mainloop: Mainloop,
    context: Context,
    pub ready: bool
}

impl Pulse
{
    pub fn new() -> Pulse
    {
        let mainloop = Mainloop::new().expect("Failed to create mainloop");
        let proplist = Proplist::new().unwrap();
        let mut context = Context::new_with_proplist(&mainloop, "mountain-keyboard-daemon", &proplist)
            .expect("Failed to create connection context");

        context.connect(None, ContextFlagSet::NOFLAGS, None).expect("Failed to connect context");

        Pulse { mainloop: mainloop, context: context, ready: false }
    }

    pub fn wait_until_ready(&mut self)
    {
        loop
        {
            match self.context.get_state() {
                libpulse_binding::context::State::Ready => break,
                libpulse_binding::context::State::Failed |
                libpulse_binding::context::State::Terminated =>
                {
                    eprintln!("Context state failed/terminated, quitting...");
                    return;
                }
                _ => { self.mainloop.iterate(false); }
            }
        }
        self.ready = true;
    }

    pub fn get_volume(&mut self) -> u8
    {
        let vol = Arc::new(Mutex::new(0_u8));
        let vol_copy = Arc::clone(&vol);
        let op = self.context.introspect().get_sink_info_list(move |info|
        {
            match info
            {
                libpulse_binding::callbacks::ListResult::Item(device) => 
                {
                    if device.state == libpulse_binding::def::SinkState::Running
                    {
                        if device.mute
                        {
                            *vol_copy.lock().unwrap() = 0
                        }
                        else 
                        {
                            let v = device.volume.max();
                            let v_f = v.0 as f32;
                            let normal = Volume::NORMAL.0 as f32;
                            let v = (v_f.div(normal) * 100.0).round() as u8;
                            *vol_copy.lock().unwrap() = v;
                        }
                    }
                }
                _ => {}
            }
        });
        while op.get_state() == libpulse_binding::operation::State::Running
        {
            self.mainloop.iterate(false);
        }
        *vol.lock().unwrap()
    }
}

impl Drop for Pulse
{
    fn drop(&mut self) 
    {
        self.context.disconnect();
        self.mainloop.quit(libpulse_binding::def::Retval(0));
    }
}
