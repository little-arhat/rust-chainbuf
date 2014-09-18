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
    let sd = dst.mut_slice(dst_ofs, dst_ofs + len);
    let ss = src.slice(src_ofs, src_ofs + len);
    let _ = sd.clone_from_slice(ss);
}

/// Chained buffer of bytes.
/// Consists of linked list of nodes.
pub struct ChbChain {
    head: DList<Box<ChbNode>>,
    length: uint
}

impl ChbChain {

    fn new() -> ChbChain {
        return ChbChain{
            head: DList::new(),
            length: 0
        }
    }

    // TODO: rename len()?
    fn size(&self) {
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
    fn add_node_tail(&mut self, node: Box<ChbNode>) {
        self.length += node.size();
        self.head.push(node);
    }

    // TODO: rename _front
    fn add_node_head(&mut self, node: Box<ChbNode>) {
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
        let node = ChbNode::new(ChbDataHolder::new(nsize)); // Box<ChbNode>
        self.add_node_tail(node);
    }

    // TODO: rename _front
    fn create_node_head(&mut self, size: uint) {
        let nsize = if size < CHB_MIN_SIZE {
            size << 1
        } else {
            size
        };
        let mut node = ChbNode::new(ChbDataHolder::new(nsize)); // Box<ChbNode>
        let r = node.room();
        node.start = r;
        node.end = r;
        self.add_node_head(node);
    }
}

/// Node of chain buffer.
/// Owned by ChbChain.
struct ChbNode {
    dh: Box<ChbDataHolder>, // можно заменить на RC
    start: uint,
    end: uint
}

impl ChbNode {
    pub fn new(dh: Box<ChbDataHolder>) -> Box<ChbNode> {
        let n = box ChbNode {
            dh: dh,
            start: 0,
            end: 0
        };
        // TODO: ref dh ? auto, when using RC
        return n;
    }

    pub fn size(&self) -> uint {
        self.end - self.start
    }

    pub fn room(&self) -> uint {
        return self.dh.size - self.end;
    }
}

/// Data holder
/// TODO: can be shared among different chains
/// TODO: implement other storages: shmem, mmap
struct ChbDataHolder{
    size: uint,
    data: Vec<u8>
}

impl ChbDataHolder {
    pub fn new(size: uint) -> Box<ChbDataHolder> {
        let dh = box ChbDataHolder {
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
