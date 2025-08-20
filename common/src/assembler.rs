use std::collections::hash_map::Entry::Vacant;
use std::collections::HashMap;

use wg_internal::{network::NodeId, packet::Fragment};


#[derive(Debug, Default)]
pub struct FragmentAssembler {
    pub fragments: HashMap<(u64, NodeId), Vec<Fragment>>, // session_id -> data buffer
    pub expected_fragments: HashMap<(u64, NodeId), u64>, // session_id -> total_fragments
    pub received_fragments: HashMap<(u64, NodeId), Vec<bool>>, // session_id -> received status
}

impl FragmentAssembler {
    pub fn add_fragment(&mut self, fragment: Fragment, session_id: u64, sender: NodeId) -> Option<Vec<u8>> {
        let communication_id = ( session_id, sender );
        #[allow(clippy::cast_possible_truncation)]
        let index = fragment.fragment_index as usize;

        if let Vacant(entry) = self.fragments.entry(communication_id) {
            self.expected_fragments.insert(communication_id, fragment.total_n_fragments);
            self.received_fragments.insert(communication_id, vec![false; index]);
            entry.insert(vec![fragment]);
        }

        {
            let received = self.received_fragments.get_mut(&communication_id)?;
            received[index] = true;
        }

        let expected = self.expected_fragments.get(&communication_id)?;
        let received = self.received_fragments.get(&communication_id)?;
        let fragments = self.fragments.get(&communication_id)?;

        // check if all fragments has been received
        if fragments.len() as u64 == *expected && received.iter().all(|f| *f){
            let fragments = self.fragments.get_mut(&communication_id)?;
            fragments.sort_by(|t, n| t.fragment_index.cmp(&n.fragment_index));
            let mut data = vec![];
            for f in fragments {
                data.copy_from_slice(&f.data);
            }
            let _ = self.fragments.remove(&communication_id);
            let _ = self.received_fragments.remove(&communication_id);
            let _ = self.expected_fragments.remove(&communication_id);
            return Some(data);
        }
        None
    }
}

