extern crate collections;

use collections::dlist::DList;
use collections::Deque;

use std::cmp;
use std::mem;

use std::rc::{mod, Rc};

static CHB_MIN_SIZE:uint = 32u;


// TODO: move to utils
fn blit<T:Clone>(src: &[T], dst: &mut [T], dst_ofs: uint) {
    let len = src.len();
    let sd = dst.slice_mut(dst_ofs, dst_ofs + len);
    if len > sd.len() {
        fail!("blit: source larger than destination");
    }

    let _ = sd.clone_from_slice(src);
}

/// Chained buffer of bytes.
/// Consists of linked list of nodes.
pub struct Chain {
    head: DList<Node>,
    length: uint
}

impl Chain {
    pub fn new() -> Chain {
        Chain{
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
                (nd.room() < size) || !rc::is_unique(&nd.dh)
            }
            None => {
                true
            }
        };
        // We either not the only owner of DH or don't have enough room
        if should_create {
            let nsize = if size < CHB_MIN_SIZE { size << 1 } else { size };
            let node = Node::new(DataHolder::new(nsize));
            self.add_node_tail(node);
        }
        // infailable: added node above
        let node = self.head.back_mut().unwrap();
        // XXX: Damn, https://github.com/rust-lang/rust/issues/6268
        // XXX: we need additional var and scope only to fight borrow checker
        {
            let node_end = node.end;
            // we should be sole owner of data holder inside node here
            let dh = rc::get_mut(&mut node.dh).unwrap();
            dh.copy_data_from(data, node_end);
        }
        node.end += size;
        self.length += size;
    }

    pub fn prepend_bytes(&mut self, data: &[u8]) {
        let size = data.len();
        // XXX: Damn, https://github.com/rust-lang/rust/issues/6393
        let should_create = match self.head.front() {
            Some(nd) => {
                (size > nd.start || !rc::is_unique(&nd.dh))
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
        // See comments in `append_bytes`
        let node = self.head.front_mut().unwrap();
        {
            let node_start = node.start;
            let dh = rc::get_mut(&mut node.dh).unwrap();
            dh.copy_data_from(data, node_start - size);
        }
        node.start -= size;
        self.length += size;
    }

    pub fn pullup(&mut self, size: uint) -> Option<&[u8]> {
        if size == 0 || size > self.len() {
            return None
        }
        // could not fail, because self.size() > 0 => has node
        if self.head.front().unwrap().size() >= size {
            let node = self.head.front().unwrap();
            return Some(node.data(size));
        }

        let mut newn = Node::new(DataHolder::new(size));
        // XXX: we need this scope to be able to move newn inside our list
        {
            let mut msize = size;
            while msize > 0 {
                {

                    let node = self.head.front_mut().unwrap();
                    let csize = cmp::min(node.size(), msize);
                    // XXX: we need this scope only to beat borrow checker
                    {
                        let node_end = newn.end;
                        // we just created new data holder, so we have unique ownership
                        let dh = rc::get_mut(&mut newn.dh).unwrap();
                        dh.copy_data_from(node.data(csize), node_end);
                    }
                    newn.end += csize;

                    if node.size() > msize {
                        node.start += msize;
                        self.length -= msize;
                        break
                    }
                }
                // infailable
                let n = self.head.pop_front().unwrap();
                self.length -= n.size();
                msize -= n.size();
            }
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

    pub fn append(&mut self, src: &Chain) {
        for node in src.head.iter() {
            self.add_node_tail(node.clone());
        }
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
    fn add_node_tail(&mut self, node: Node) {
        self.length += node.size();
        self.head.push(node);
    }

    fn add_node_head(&mut self, node: Node) {
        self.length += node.size();
        self.head.push_front(node);
    }
}

/// Node of chain buffer.
/// Owned by Chain.
struct Node {
    dh: Rc<DataHolder>,
    start: uint,
    end: uint
}

impl Node {
    fn new(dh: Rc<DataHolder>) -> Node {
        Node {
            dh: dh,
            start: 0,
            end: 0
        }
    }

    fn size(&self) -> uint {
        self.end - self.start
    }

    fn room(&self) -> uint {
        self.dh.size - self.end
    }

    fn data(&self, size:uint) -> &[u8] {
        self.dh.data.slice(self.start, self.start + size)
    }

}

impl Clone for Node {
    fn clone(&self) -> Node {
        let mut newn = Node::new(self.dh.clone());
        newn.start = self.start;
        newn.end = self.end;
        newn
    }
}

/// Refcounted data holder
/// TODO: can be shared among different chains
/// TODO: implement other storages: shmem, mmap
struct DataHolder{
    size: uint,
    data: Vec<u8>
}

impl DataHolder {
    fn new(size: uint) -> Rc<DataHolder> {
        Rc::new(DataHolder {
            size: size,
            data: Vec::from_elem(size, 0)
        })
    }

    fn copy_data_from(&mut self, src: &[u8], dst_offs: uint) {
        blit(src,
             self.data.as_mut_slice(), dst_offs);
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
            buf.extend(b.iter().map(|x| x.clone()));
            chain.append_bytes(b);
            t -= one_seq;
        }
        {
            let ret = chain.pullup(total);
            assert!(ret.is_some());
            assert_eq!(ret.unwrap(), buf.as_slice());
        }
        assert_eq!(chain.len(), total);
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

    #[test]
    fn test_append_copies_data() {
        let mut chain1 = Chain::new();
        let mut chain2 = Chain::new();
        let s = "HelloWorld";
        let b = s.as_bytes();
        let lb = b.len();
        let mut ss = String::from_str(s);
        ss.push_str(s);
        chain1.append_bytes(b);
        chain2.append_bytes(b);
        chain1.append(&mut chain2);
        {
            let res = chain1.pullup(2 * lb);
            assert!(res.is_some());
            assert_eq!(res.unwrap(), ss.as_bytes());
        }
        assert_eq!(chain1.len(), 2 * lb);
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
