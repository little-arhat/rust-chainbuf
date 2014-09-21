// TODO: use Impls https://github.com/iron/iron/blob/master/src/iron.rs

extern crate collections;

use collections::dlist::DList;
use collections::Deque;

static CHB_MIN_SIZE:uint = 32u;


// TODO: move to utils
fn blit<T:Clone>(src: &[T], src_ofs: uint, dst: &mut [T], dst_ofs: uint, len: uint) {
    if (src_ofs > src.len() - len) || (dst_ofs > dst.len() - len) {
        fail!("blit: invalid argument!");
    }
    let sd = dst.slice_mut(dst_ofs, dst_ofs + len);
    let ss = src.slice(src_ofs, src_ofs + len);
    let _ = sd.clone_from_slice(ss);
}

/// Chained buffer of bytes.
/// Consists of linked list of nodes.
pub struct Chain {
    head: DList<Box<Node>>,
    length: uint
}

impl Chain {
    pub fn new() -> Chain {
        return Chain{
            head: DList::new(),
            length: 0
        }
    }

    // TODO: rename len()?
    pub fn size(&self) -> uint {
        self.length
    }

    // XXX: maybe DEDUP append/prepend?
    // TODO: test: length, capacity, node size
    pub fn append_bytes(&mut self, data: &[u8]) {
        let size = data.len();
        // XXX: Damn, https://github.com/rust-lang/rust/issues/6393
        let should_create = match self.head.back() {
            Some(nd) => {
                // Check is READONLY
                nd.room() < size
            }
            None => {
                true
            }
        };
        if should_create {
            let nsize = if size < CHB_MIN_SIZE { size << 1 } else { size };
            let node = Node::new(DataHolder::new(nsize)); // Box<Node>
            self.add_node_tail(node);
        }
        // node could not be None here
        let node = self.head.back_mut().unwrap();
        node.append_from(data, 0, size);
        self.length += size;
    }

    // TODO: test: length, capacity, node size
    pub fn prepend_bytes(&mut self, data: &[u8]) {
        let size = data.len();
        // XXX: Damn, https://github.com/rust-lang/rust/issues/6393
        let should_create = match self.head.front() {
            Some(nd) => {
                // Check is READONLY
                size > nd.start
            }
            None => {
                true
            }
        };
        if should_create {
            let nsize = if size < CHB_MIN_SIZE { size << 1 } else { size };
            let mut node = Node::new(DataHolder::new(nsize)); // Box<Node>
            let r = node.room();
            node.start = r;
            node.end = r;
            self.add_node_head(node);
        }
        // node could not be None here
        let node = self.head.front_mut().unwrap();
        node.prepend_from(data, 0, size);
        self.length += size;
    }

    pub fn pullup(&mut self, size: uint) -> Option<&[u8]> {
        if size == 0 || size > self.size() {
            return None
        }
        // let node = match chb.head
        None
    }

    // XXX: private
    fn add_node_tail(&mut self, node: Box<Node>) {
        self.length += node.size();
        self.head.push(node);
    }

    fn add_node_head(&mut self, node: Box<Node>) {
        self.length += node.size();
        self.head.push_front(node);
    }
}

/// Node of chain buffer.
/// Owned by Chain.
struct Node {
    dh: Box<DataHolder>, // можно заменить на RC
    start: uint,
    end: uint
}

impl Node {
    fn new(dh: Box<DataHolder>) -> Box<Node> {
        let n = box Node {
            dh: dh,
            start: 0,
            end: 0
        };
        // TODO: ref dh ? auto, when using RC
        return n;
    }

    fn size(&self) -> uint {
        self.end - self.start
    }

    fn room(&self) -> uint {
        self.dh.size - self.end
    }

    fn data(&self) -> &[u8] {
        self.dh.data.as_slice()
    }

    fn data_range(&self, start:uint, end:uint) -> &[u8] {
        self.data().slice(start, end)
    }

    fn mut_data(&mut self) -> &mut [u8] {
        self.dh.data.as_mut_slice()
    }

    fn append_from(&mut self, data: &[u8], offs: uint, size: uint) {
        // XXX: Damn, https://github.com/rust-lang/rust/issues/6268
        let e = self.end;
        blit(data, offs,
             self.mut_data(), e,
             size);
        self.end += size
    }

    fn prepend_from(&mut self, data: &[u8], offs: uint, size: uint) {
        let s = self.start;
        blit(data, offs,
             self.mut_data(), s - size,
             size);
        self.start -= size;
    }
}

/// Data holder
/// TODO: can be shared among different chains
/// TODO: implement other storages: shmem, mmap
struct DataHolder{
    size: uint,
    data: Vec<u8>
}

impl DataHolder {
    fn new(size: uint) -> Box<DataHolder> {
        let dh = box DataHolder {
            size: size,
            data: Vec::from_elem(size, 0)
        };
        return dh;
    }
}

#[cfg(test)]
mod test {
    use super::Chain;

    #[test]
    fn test_append_bytes_changes_length() {
        let mut chain = Chain::new();
        let s = "HelloWorld";
        let ls = s.len();
        chain.append_bytes(s.as_bytes());
        assert_eq!(chain.size(), ls);
    }

    #[test]
    fn test_prepend_bytes_changes_length() {
        let mut chain = Chain::new();
        let s = "HelloWorld";
        let ls = s.len();
        chain.prepend_bytes(s.as_bytes());
        assert_eq!(chain.size(), ls);
    }

}
