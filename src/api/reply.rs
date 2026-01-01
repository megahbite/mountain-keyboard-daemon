#[derive(PartialEq)]
pub enum ReplyType
{
    Unknown,
    Keepalive,
    Handshake,
    TimeUpdate
}

pub enum ReplyParseError
{
    TypeMismatch
}

pub struct ReplyPacket 
{
    pub reply_type: ReplyType,
    #[allow(dead_code)]
    pub data: [u8; 62]
}

#[derive(PartialEq, Clone)]
pub enum CommandState 
{
    Idle,
    SendHandshake,
    CheckTimeUpdate,
    SendTimeUpdate,
    SendVolume
}

impl ReplyPacket
{
    pub fn from_buf(buf: &[u8; 64]) -> ReplyPacket
    {
        let mut data = [0u8; 62];
        data.copy_from_slice(&buf[2..]);
        let reply_type = match buf[0]
        {
            0x11 => 
            {
                match buf[1]
                {
                    0x14 => ReplyType::Keepalive,
                    0x80 => ReplyType::Handshake,
                    0x84 => ReplyType::TimeUpdate,
                    _ => ReplyType::Unknown
                }
            },
            _ => ReplyType::Unknown
        };
        ReplyPacket{ reply_type: reply_type, data: data }
    }
}

pub struct TimeUpdateReply
{
    pub needs_update: bool,
    pub update_ack: bool
}

impl TimeUpdateReply
{
    pub fn parse_reply(reply: ReplyPacket) -> Result<TimeUpdateReply, ReplyParseError>
    {
        if reply.reply_type != ReplyType::TimeUpdate
        {
            Err(ReplyParseError::TypeMismatch)
        }
        else 
        {
            Ok(TimeUpdateReply { needs_update: reply.data[9] == 0x01, update_ack: reply.data[1] == 0x01 })
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum DisplayState
{
    Unknown,
    Clock,
    ClockTimeAndDate,
    ClockStopwatch,
    ClockSetTimer,
    ClockShowTimeAndDate,
    Volume,
    CPU,
    GPU
}

impl TryFrom<u8> for DisplayState 
{
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> 
    {
        match value
        {
            0x10 => Ok(DisplayState::Clock),
            0x11 => Ok(DisplayState::ClockTimeAndDate),
            0x12 => Ok(DisplayState::ClockStopwatch),
            0x13 => Ok(DisplayState::ClockSetTimer),
            0x15 => Ok(DisplayState::ClockShowTimeAndDate),
            0x41 => Ok(DisplayState::Volume),
            0x61 => Ok(DisplayState::CPU),
            0x62 => Ok(DisplayState::GPU),
            _ => Err(())
        }
    }
}

pub struct KeepaliveReply
{
    pub media_dock_present: bool,
    pub display_state: DisplayState,
    pub num_connected_accessories: u8,
}

impl KeepaliveReply
{
    pub fn parse_reply(reply: ReplyPacket) -> Result<KeepaliveReply, ReplyParseError>
    {
        if reply.reply_type != ReplyType::Keepalive
        {
            Err(ReplyParseError::TypeMismatch)
        }
        else 
        {
            let media_dock_present = reply.data[2] == 0x01;
            let display_state = reply.data[4].try_into().unwrap_or(DisplayState::Unknown);
            println!("{:x} {:#?}", reply.data[4], display_state);
            let num_connected_accessories = reply.data[18];

            Ok(KeepaliveReply {
                media_dock_present: media_dock_present,
                display_state: display_state,
                num_connected_accessories: num_connected_accessories
            })
        }
    }

    #[allow(dead_code)]
    pub fn is_numpad_present(&self) -> bool
    {
        (self.media_dock_present && self.num_connected_accessories > 1) || 
            (!self.media_dock_present && self.num_connected_accessories > 0)
    }
}

pub fn handle_reply(reply: ReplyPacket, initialised: &mut bool, last_display_state: &mut DisplayState) -> CommandState
{
    let command_state = match reply.reply_type
    {
        ReplyType::Keepalive => 
        {
            let mut state = CommandState::Idle;
            if let Ok(keepalive) = KeepaliveReply::parse_reply(reply)
            {
                if !*initialised
                {
                    *initialised = true;
                    state = CommandState::SendHandshake;
                }
                else
                {
                    state = match keepalive.display_state
                    {
                        DisplayState::ClockShowTimeAndDate =>
                        {
                            if *last_display_state != DisplayState::ClockShowTimeAndDate
                            {
                                CommandState::CheckTimeUpdate
                            }
                            else
                            {
                                CommandState::Idle
                            }
                        }
                        DisplayState::Volume => CommandState::SendVolume,
                        _ => CommandState::Idle
                    }
                }
                *last_display_state = keepalive.display_state;
            }
            state
        },
        ReplyType::Handshake => CommandState::CheckTimeUpdate,
        ReplyType::TimeUpdate => 
        {
            let mut state = CommandState::Idle;
            if let Ok(time_update) = TimeUpdateReply::parse_reply(reply) && time_update.needs_update && !time_update.update_ack
            {
                state = CommandState::SendTimeUpdate;
            }
            state
        },
        _ => CommandState::Idle
    };
    command_state
}
