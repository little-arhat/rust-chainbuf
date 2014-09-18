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
    pub fn append_bytes(mut self, data: &[u8]) {
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
            self.create_node_tail(size);
        }
        // node could not be None here
        let node = self.head.back_mut().unwrap();
        // XXX: Damn, https://github.com/rust-lang/rust/issues/6268
        let end = node.end;
        blit(data.as_slice(), 0,
             node.dh.data.as_mut_slice(), end,
             size);
        node.end += size;
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
            self.create_node_head(size);
        }
        // node could not be None here
        let node = self.head.front_mut().unwrap();
        // XXX: Damn, https://github.com/rust-lang/rust/issues/6268
        let start = node.start;
        blit(data.as_slice(), 0,
             node.dh.data.as_mut_slice(), start - size,
             size);
        node.start -= size;
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

    // TODO: rename _back
    fn add_node_tail(&mut self, node: Box<Node>) {
        self.length += node.size();
        self.head.push(node);
    }

    // TODO: rename _front
    fn add_node_head(&mut self, node: Box<Node>) {
        self.length += node.size();
        self.head.push_front(node);
    }

    // TODO: rename _back
    // TODO: remove?
    fn create_node_tail(&mut self, size: uint) {
        let nsize = if size < CHB_MIN_SIZE {
            size << 1
        } else {
            size
        };
        let node = Node::new(DataHolder::new(nsize)); // Box<Node>
        self.add_node_tail(node);
    }

    // TODO: rename _front
    fn create_node_head(&mut self, size: uint) {
        let nsize = if size < CHB_MIN_SIZE {
            size << 1
        } else {
            size
        };
        let mut node = Node::new(DataHolder::new(nsize)); // Box<Node>
        let r = node.room();
        node.start = r;
        node.end = r;
        self.add_node_head(node);
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
        return self.dh.size - self.end;
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

// TODO: move to test
// let mut chain = Chb::new();
// chain.append_bytes("abcdefghijklmnop".as_bytes());
// chain.prepend_bytes("xxx".as_bytes());
