
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