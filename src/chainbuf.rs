// TODO: use Impls https://github.com/iron/iron/blob/master/src/iron.rs

extern crate collections;

use collections::dlist::DList;
use collections::Deque;

use std::cmp;
use std::mem;

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

    pub fn from_foreign(src: Chain) -> Chain {
        let mut ch = Chain::new();
        ch.concat(src);
        ch
    }

    pub fn len(&self) -> uint {
        self.length
    }

    // XXX: maybe DEDUP append/prepend?
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
        // infailable: added node above
        let node = self.head.back_mut().unwrap();
        node.append_from(data, 0, size);
        self.length += size;
    }

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
        if size == 0 || size > self.len() {
            return None
        }
        // could not fail, because self.size() > 0 => has node
        if self.head.front().unwrap().size() >= size {
            let node = self.head.front().unwrap();
            return Some(node.data_range(node.start, node.start + size));
        }

        let mut newn = Node::new(DataHolder::new(size));
        let mut msize = size;
        while msize > 0 {
            {
                let node = self.head.front_mut().unwrap();
                let csize = cmp::min(node.size(), size);
                newn.append_from(node.data(), node.start, csize);

                if node.size() > size {
                    node.start += size;
                    self.length -= size;
                    break
                }
            }
            // infailable
            let n = self.head.pop_front().unwrap();
            msize -= n.size();
            // XXX: free node?
        }
        self.add_node_head(newn);
        // Now first node.size >= size, so we recurse
        return self.pullup(size)
    }

    pub fn concat(&mut self, src: Chain) {
        self.length += src.length;
        self.head.append(src.head);
        // No need to cleanup `src`, because it has moved and cannot be used
    }

    // XXX: chb_drop; `drop` is the sole method of built-in Drop trait,
    // so use another name
    pub fn reset(&mut self) {
        self.head = DList::new();
        self.length = 0;
    }

    // XXX: deprecated & experimental

    // TODO: maybe we do not need this method?
    // TODO: `concat` better express move semantics
    // XXX: to delete...
    pub fn concat_ptr(&mut self, src: &mut Chain) {
        self.length += src.length;
        let sh = mem::replace(&mut src.head, DList::new());
        self.head.append(sh);
        src.length = 0;
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
    use std::rand::{task_rng, Rng};

    #[test]
    fn test_append_bytes_changes_length() {
        let mut chain = Chain::new();
        let s = "HelloWorld";
        let ls = s.len();
        chain.append_bytes(s.as_bytes());
        assert_eq!(chain.len(), ls);
    }

    #[test]
    fn test_prepend_bytes_changes_length() {
        let mut chain = Chain::new();
        let s = "HelloWorld".as_bytes();
        let ls = s.len();
        chain.prepend_bytes(s);
        assert_eq!(chain.len(), ls);
    }

    #[test]
    fn test_from_foreign_moves_all_data() {
        let mut orig = Chain::new();
        orig.append_bytes("HelloWorld".as_bytes());
        let sz = orig.len();
        let new = Chain::from_foreign(orig);
        assert_eq!(new.len(), sz);
    }

    #[test]
    fn test_pullup_return_none_on_empty_chain() {
        let mut chain = Chain::new();
        assert!(chain.pullup(1).is_none());
    }

    #[test]
    fn test_pullup_return_what_has_been_appended() {
        let mut chain = Chain::new();
        let s = "HelloWorld".as_bytes();
        let ls = s.len();
        chain.append_bytes(s);
        let res = chain.pullup(ls);
        assert!(res.is_some());
        assert_eq!(res.unwrap(), s);
    }

    #[test]
    fn test_pullup_does_not_change_length() {
        let mut chain = Chain::new();
        let s = "HelloWorld".as_bytes();
        let ls = s.len();
        chain.append_bytes(s);
        let olds = chain.len();
        chain.pullup(ls / 2);
        assert_eq!(chain.len(), olds);
    }

    #[test]
    fn test_pullup_works_on_large_sequences() {
        let mut chain = Chain::new();
        let total = 2048u;
        let mut t = total;
        let one_seq = 128u;
        let mut buf = Vec::new();
        while t > 0 {
            let s:String = task_rng().gen_ascii_chars().take(one_seq).collect();
            let b = s.as_bytes();
            buf = buf.append(b);
            chain.append_bytes(b);
            t -= one_seq;
        }
        let ret = chain.pullup(total);
        assert!(ret.is_some());
        assert_eq!(ret.unwrap(), buf.as_slice());
    }

    #[test]
    fn test_concat_increase_dst_length() {
        let mut chain1 = Chain::new();
        let mut chain2 = Chain::new();
        chain1.append_bytes("HelloWorld".as_bytes());
        let l1 = chain1.len();
        chain2.append_bytes("HelloWorld".as_bytes());
        let l2 = chain2.len();
        chain1.concat(chain2);
        assert_eq!(chain1.len(), l1+l2);
    }

    #[test]
    fn test_concat_appends_content() {
        let mut chain1 = Chain::new();
        let mut chain2 = Chain::new();
        let b = "HelloWorld".as_bytes();
        let bl = b.len();
        chain2.append_bytes(b);
        chain1.concat(chain2);
        let res = chain1.pullup(bl);

        assert!(res.is_some());
        assert_eq!(res.unwrap(), b);
    }

    #[test]
    fn test_reset_empties_chain() {
        let mut chain = Chain::new();
        chain.append_bytes("HelloWorld".as_bytes());
        chain.reset();
        assert!(chain.pullup(1).is_none());
        assert_eq!(chain.len(), 0);
    }

    // XXX: do not need to test it for `concat`, because it is done for us
    // XXX: by type-system.
    #[test]
    fn test_concat_ptr_destroy_src() {
        let mut chain1 = Chain::new();
        let mut chain2 = Chain::new();
        chain2.append_bytes("HelloWorld".as_bytes());
        chain1.concat_ptr(&mut chain2);
        assert_eq!(chain2.len(), 0);
    }

}
