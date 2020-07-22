use serde::{Deserialize, Serialize};

pub trait AsIpcPacket {
    fn timestamp(&self) -> &std::time::SystemTime;
    fn data(&self) -> Vec<u8>;
}

#[derive(Debug, Deserialize, Serialize)]
pub struct IpcPacket {
    timestamp: std::time::SystemTime,
    #[serde(with = "serde_bytes")]
    data: Vec<u8>,
}

impl<T: AsIpcPacket> From<&T> for IpcPacket {
    fn from(v: &T) -> Self {
        IpcPacket {
            timestamp: v.timestamp().clone(),
            data: v.data(),
        }
    }
}

impl From<IpcPacket> for Packet {
    fn from(v: IpcPacket) -> Self {
        Packet {
            ts: v.timestamp.clone(),
            data: v.data.to_vec(),
        }
    }
}

#[derive(Debug)]
pub struct Packet {
    ts: std::time::SystemTime,
    data: Vec<u8>,
}

impl Packet {
    pub fn new(ts: std::time::SystemTime, data: Vec<u8>) -> Packet {
        Packet { ts, data }
    }

    pub fn into_data(self) -> Vec<u8> {
        self.data
    }
}

impl AsIpcPacket for Packet {
    fn timestamp(&self) -> &std::time::SystemTime {
        &self.ts
    }
    fn data(&self) -> Vec<u8> {
        self.data.clone()
    }
}

#[macro_export]
macro_rules! impl_ipc_serialize(
    (
        $t:ident
    ) => {
        impl serde::Serialize for $t {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                let packet = IpcPacket::from(self);
                packet.serialize(serializer)
            }
        }
    }
);

impl_ipc_serialize!(Packet);

impl<'de> Deserialize<'de> for Packet {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let ipc_packet = IpcPacket::deserialize(deserializer)?;
        Ok(ipc_packet.into())
    }
}
