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

fn capture_into_enum(key_name: String) -> () {
    || ChainRequest::AddKey {
        key_name
    };
}

pub fn max_by_key<A, B: Ord>(a: A, b: A, key: impl Fn(&A) -> B) -> A {
    if key(&a) > key(&b) {
        a
    } else {
        b
    }
}

fn higher_ranked_closure() {
    let _ = (0..1).filter(|_| true);
}
