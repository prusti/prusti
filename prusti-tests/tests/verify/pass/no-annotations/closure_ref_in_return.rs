pub enum TrackingId {
    Static(&'static str),
}

pub struct TrackedMsgs {
    pub msgs: u32,
    pub tracking_id: TrackingId
}

pub enum ChainRequest {
    SendMessagesAndWaitCheckTx {
        tracked_msgs: TrackedMsgs
    },

    AddKey {
        key_name: String
    },
}


fn go(key_name: String) -> () {
    || ChainRequest::AddKey {
        key_name
    };
}
