extern crate collections;

use collections::dlist::DList;
use collections::Deque;
use collections::slice::bytes;

use std::cmp;
use std::str;
use std::mem;

use std::rc::{mod, Rc};

pub static CHB_MIN_SIZE:uint = 32u;



/// Move at most n items from the front of src deque to thes back of
/// dst deque.
// XXX: if we had access to DList impl, we could do this more effective
fn move_n<TT, T: Deque<TT>>(src: &mut T, dst: &mut T, n: uint) {
    let mut nc = n;
    while nc > 0 {
        if let Some(el) = src.pop_front() {
            dst.push(el);
            nc -= 1;
        } else {
            break;
        }
    }
}

/// Finds subsequence of bytes in bytesequence. Returnt offset or None
/// if nothing found.
fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<uint> {
    unsafe {
        let hs:&str = mem::transmute(haystack);
        let ns:&str = mem::transmute(needle);
        hs.find_str(ns)
    }
}

fn find_overlap<U:Eq, T:Iterator<U> + Clone + Copy>(large: T, short: T) -> uint {
    let mut haystack_it = large;
    let mut needle_it = short;
    let mut matched = 0u;
    let mut current_needle = needle_it;
    loop {
        if let Some(b) = current_needle.next() {
            if let Some(h) = haystack_it.next() {
                if b == h {
                    // save position of iter for backtracking
                    needle_it = current_needle;
                    current_needle = current_needle.clone();
                    matched += 1;
                } else {
                    // match failed, if we have previous matches,
                    // restore iter
                    if matched > 0 {
                        current_needle = needle_it;
                    }
                    // restore haystack iter
                    haystack_it = large;
                    matched = 0;
                }
            } else {
                // haystack exhausted
                haystack_it = large;
            }
        } else {
            break;
        }
    }
    return matched;
}


/// Chained buffer of bytes.
/// # Examples:
/// ```
/// use chainbuf::Chain;
/// let mut chain = Chain::new();
/// chain.append_bytes("helloworld".as_bytes());
/// let some_bytes = chain.pullup(2);
/// ```
/// # Details of implementation
/// Chainbuf consists of linked list of nodes, with `start` and `end`
/// offsets and a reference counted pointer to DataHolder. DataHolders can be
/// shared across different chains, so for mutation new nodes and data holders
/// are created (as in Copy-On-Write).
pub struct Chain {
    head: DList<Node>,
    length: uint
}

struct NodeAtPosInfoMut<'a> {
    node: &'a mut Node, // link to node
    pos: uint, // position of node in chain
    offset: uint // offset inside node
}

struct NodeAtPosInfo<'a> {
    node: &'a Node, // link to node
    pos: uint, // position of node in chain
    offset: uint // offset inside node
}


impl Chain {
    /// Creates new, empty chainbuf.
    /// Chainbuf will not allocate any nodes until something are
    /// pushed onto it.
    /// # Example
    /// ```
    /// use chainbuf::Chain;
    /// let mut chain = Chain::new();
    /// ```
    pub fn new() -> Chain {
        Chain{
            head: DList::new(),
            length: 0
        }
    }

    /// Constructs new chainbuf from another chainbuf, destroying it.
    /// # Example
    /// ```
    /// use chainbuf::Chain;
    /// let mut chain1 = Chain::new();
    /// chain1.append_bytes("helloworld".as_bytes());
    /// let mut chain2 = Chain::from_foreign(chain1);
    /// println!("{}", chain2.len()); // should print 10
    /// // println!("{}", chain1.len()); // should produce error `use of moved value`
    /// ```
    pub fn from_foreign(src: Chain) -> Chain {
        let mut ch = Chain::new();
        ch.concat(src);
        ch
    }

    /// Returns number of bytes stored in chainbuf.
    /// # Example
    /// ```
    /// use chainbuf::Chain;
    /// let mut chain = Chain::new();
    /// chain.append_bytes("helloworld".as_bytes());
    /// println!("{}", chain.len()); // should print 10
    /// ```
    #[inline]
    pub fn len(&self) -> uint {
        self.length
    }

    /// Copies bytes from a slice, and appends them to the end of chain,
    /// creating new node, if data holder in last node does not have enough
    /// room for data or shared across several chains.
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

    /// Copies bytes from a slice, and prepends them to the begining of chain,
    /// creating new node, if data holder in last node does not have enough
    /// room for data or shared across several chains.
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

    /// Returns slice of data of requested size or None,
    /// if chain does not have enough data.
    /// If data of requested size span multiple nodes, new node, containing
    /// all requested data will be created instead.
    /// # Example
    /// ```
    /// use chainbuf::Chain;
    /// let mut chain = Chain::new();
    /// chain.append_bytes("helloworld".as_bytes()); // new node created
    /// chain.append_bytes("helloworldhelloworld".as_bytes()); // new node created
    /// assert_eq!(chain.pullup(100), None);
    /// assert_eq!(chain.pullup(2).unwrap(), "he".as_bytes()); // does not create new node
    /// assert_eq!(chain.pullup(25).unwrap(), "helloworldhelloworldhello".as_bytes()); // create new node
    /// ```
    pub fn pullup(&mut self, size: uint) -> Option<&[u8]> {
        if size == 0 || size > self.len() {
            return None
        }
        // could not fail, because self.size() > 0 => has node
        if self.head.front().unwrap().size() >= size {
            let node = self.head.front().unwrap();
            return Some(node.get_data_from_start(size));
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
                        dh.copy_data_from(node.get_data_from_start(csize),
                                          node_end);
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

    /// Returns slice of requested size starting from specified offset.
    /// # Example
    /// ```
    /// use chainbuf::Chain;
    /// let mut chain = Chain::new();
    /// chain.append_bytes("helloworld".as_bytes());
    /// let res = chain.pullup_from(2, 4);
    /// assert!(res.is_some());
    /// assert_eq!(res.unwrap(), "llow".as_bytes());
    /// ```
    pub fn pullup_from(&mut self, offs: uint, size: uint) -> Option<&[u8]> {
        if (offs >= self.len()) || (size == 0) {
            return None;
        }
        let mut tmp = Chain::new();
        tmp.move_from(self, offs);
        // Run pullup to be sure, that we have dataholder that contains
        // requested number of bytes in contigious memory
        let _ = self.pullup(size);
        tmp.move_all_from(self);
        self.concat(tmp);
        // We've done sanity check, so can safely unwrap this:
        let node_info = self.node_at_pos(offs).unwrap();
        // This node will contain requested data from the start
        return Some(node_info.node.get_data_from_start(size));
    }

    /// Shortcut for chain.pullup(chain.len()).
    /// # Example
    /// ```
    /// use chainbuf::Chain;
    /// let mut chain = Chain::new();
    /// chain.append_bytes("helloworld".as_bytes());
    /// let buf = chain.pullup_all();
    /// assert_eq!(buf.unwrap().len(), 10);
    /// ```
    pub fn pullup_all(&mut self) -> Option<&[u8]> {
        let l = self.len();
        self.pullup(l)
    }

    /// Pulls all data and returns it as utf8 str or None if chain is empty
    /// or contains invalid utf8 data.
    /// # Example
    /// ```
    /// use chainbuf::Chain;
    /// let mut chain = Chain::new();
    /// chain.append_bytes("helloworld".as_bytes());
    /// let res = chain.to_utf8_str();
    /// assert!(res.is_some());
    /// assert_eq!(res.unwrap(), "helloworld");
    /// ```
    pub fn to_utf8_str(&mut self) -> Option<&str> {
        match self.pullup_all() {
            Some(bytes) => { str::from_utf8(bytes) }
            None => { None }
        }
    }

    /// Consumes another chain and moves all data from it to itself.
    /// # Example
    /// ```
    /// use chainbuf::Chain;
    /// let mut chain1 = Chain::new();
    /// let mut chain2 = Chain::new();
    /// chain1.append_bytes("hello".as_bytes());
    /// chain2.append_bytes("world".as_bytes());
    /// chain1.concat(chain2);
    /// assert_eq!(chain1.pullup(10).unwrap(), "helloworld".as_bytes());
    /// ```
    pub fn concat(&mut self, src: Chain) {
        self.length += src.length;
        self.head.append(src.head);
        // No need to cleanup `src`, because it has moved and cannot be used
    }

    /// Discards all data in chain, deletes all nodes and set length to 0.
    /// # Example:
    /// ```
    /// use chainbuf::Chain;
    /// let mut chain = Chain::new();
    /// chain.append_bytes("helloworld".as_bytes());
    /// assert_eq!(chain.len(), 10);
    /// chain.reset();
    /// assert_eq!(chain.len(), 0);
    /// ```
    pub fn reset(&mut self) {
        // XXX: chb_drop; `drop` is the sole method of built-in Drop trait,
        // so use another name
        self.head = DList::new();
        self.length = 0;
    }

    /// Appends data from another chain to itself.
    /// # Note
    /// This method creates new nodes with same offsets and pointer as in
    /// src node. No data copy happens.
    /// # Example
    /// ```
    /// use chainbuf::Chain;
    /// let mut chain1 = Chain::new();
    /// let mut chain2 = Chain::new();
    /// chain2.append_bytes("helloworld".as_bytes());
    /// chain1.append(&chain2);
    /// assert_eq!(chain1.len(), chain2.len());
    /// ```
    pub fn append(&mut self, src: &Chain) {
        // XXX: chb_copy
        for node in src.head.iter() {
            self.add_node_tail(node.clone());
        }
    }

    /// Moves at most size bytes from another chain and returns number of
    /// bytes moved.
    /// # Example
    /// ```
    /// use chainbuf::Chain;
    /// let mut chain1 = Chain::new();
    /// let mut chain2 = Chain::new();
    /// chain1.append_bytes("helloworld".as_bytes());
    /// let moved = chain2.move_from(&mut chain1, 3);
    /// assert_eq!(moved, 3);
    /// let moved_more = chain2.move_from(&mut chain1, 10);
    /// assert_eq!(moved_more, 7);
    /// ```
    pub fn move_from(&mut self, src: &mut Chain, size: uint) -> uint {
        if size == 0 {
            return 0;
        }
        if size >= src.len() {
            let sz = src.len();
            self.move_all_from(src);
            return sz;
        }

        let mut move_nodes;
        let mut newn = None;
        // We've done checks, so we cannot have None here
        {
            let node_info = src.node_at_pos_mut(size).unwrap();
            if node_info.offset != 0 {
                // We requesting data in the middle of node, should split it then
                let mut nn = node_info.node.clone();
                nn.start += node_info.offset;
                node_info.node.end = nn.start;
                newn = Some(nn);
                move_nodes = node_info.pos + 1;
            } else {
                // Requested data right on the border of nodes, can move all nodes
                // before this one
                move_nodes = node_info.pos;
            }
        }
        move_n(&mut src.head, &mut self.head, move_nodes);
        if newn.is_some() {
            src.head.push_front(newn.unwrap());
        }

        self.length += size;
        src.length -= size;

        return size;
    }

    /// Moves all data from sourche chain to itself.
    ///
    /// This operation should compute in O(1).
    /// # Example
    /// ```
    /// use chainbuf::Chain;
    /// let mut chain1 = Chain::new();
    /// let mut chain2 = Chain::new();
    /// chain1.append_bytes("helloworld".as_bytes());
    /// chain2.move_all_from(&mut chain1);
    /// assert_eq!(chain1.len(), 0);
    /// assert_eq!(chain2.len(), 10);
    /// ```
    pub fn move_all_from(&mut self, src: &mut Chain) {
        self.length += src.length;
        let sh = mem::replace(&mut src.head, DList::new());
        self.head.append(sh);
        src.length = 0;
    }

    /// Returns mutable slice of requested size that points to empty area in
    /// DataHolder. If requested size greater than available room in
    /// existing node, new node will be created.
    /// # Usage
    /// After writing data to buffer .written(size) should be calling
    /// to move offsets.
    /// # Example
    /// ```
    /// use chainbuf::Chain;
    /// let mut chain = Chain::new();
    /// let buf = chain.reserve(10);
    /// assert_eq!(buf.len(), 10);
    /// ```
    pub fn reserve(&mut self, size: uint) -> &mut [u8] {
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
        // infailable: have node, or have added it above
        let node = self.head.back_mut().unwrap();
        let dh = rc::get_mut(&mut node.dh).unwrap();
        dh.get_data_mut(node.end, size)
    }

    /// Changes offsets in chain to specified number of bytes.
    /// Should be used in conjuction with .reserve().
    /// # Example
    /// ```
    /// use chainbuf::Chain;
    /// let mut chain = Chain::new();
    /// {
    ///     let buf = chain.reserve(2);
    ///     buf[0] = 'h' as u8;
    ///     buf[1] = 'i' as u8;
    /// }
    /// chain.written(2);
    /// assert_eq!(chain.len(), 2);
    /// ```
    pub fn written(&mut self, size: uint) {
        // XXX: think, now we can enforce correct usage of reserve/written
        // XXX: with type-system?
        // XXX: for now, it's responsibility of user to use this API correctly
        // TODO: mark as unsafe API? (it's only (sic!) logically unsafe, though)
        let node = self.head.back_mut().unwrap();
        node.end += size;
        self.length += size;
    }

    /// Removes requested number of bytes from chain, by changing offsets.
    /// # Note
    /// If requested size greater than size of node it will be removed
    /// and data will be fred if no other chain shares it.
    /// # Example
    /// ```
    /// use chainbuf::Chain;
    /// let mut chain = Chain::new();
    /// chain.append_bytes("somebinaryprotocol".as_bytes());
    /// {
    ///     let head = chain.pullup(2); // parse header
    /// }
    /// chain.drain(2); // header parsed and no longer needed
    /// assert_eq!(chain.len(), 16);
    /// ```
    pub fn drain(&mut self, size: uint) {
        let mut msize = size;
        while msize > 0 {
            {
                let node = match self.head.front_mut() {
                    Some(nd) => { nd }
                    None => { break }
                };
                if node.size() > size {
                    node.start += size;
                    self.length -= size;
                    break;
                }
            }
            // infailable
            let node = self.head.pop_front().unwrap();
            msize -= node.size();
        }
    }

    /// Finds sequence of bytes inside the chain and returns offset to
    /// first symbol of sequence or None if nothing found.
    /// # Example
    /// ```
    /// use chainbuf::Chain;
    /// let mut chain = Chain::new();
    /// chain.append_bytes("helloworld".as_bytes());
    /// let res = chain.find("owo".as_bytes());
    /// assert!(res.is_some());
    /// assert_eq!(res.unwrap(), 4);
    /// ```
    pub fn find(&self, needle: &[u8]) -> Option<uint> {
        let mut msum = 0;
        let mut work_needle = needle;
        for n in self.head.iter() {
            // Try to find entire needle in one node
            let node_data = n.get_data_from_start(n.size());
            if let Some(offs) = find_bytes(node_data, work_needle) {
                return Some(msum + offs);
            } else {
                // Entire needle wasn't found, maybe suffix of data is
                // prefix of needle?
                // Find number of overlaped bytes
                let overlaped = if node_data.len() > work_needle.len() {
                    // If node_data larger than needle we gonna search
                    // from the back, looking whether some prefix of needle
                    // equal to the suffix of node_data
                    find_overlap(node_data.iter().rev(),
                                 work_needle.iter().rev())
                } else {
                    // Otherwase, we're searching for suffix of node_data
                    // that equal to some prefix of the needle
                    find_overlap(work_needle.iter(),
                                 node_data.iter())
                };

                if overlaped > 0 {
                    // if we found something, move offset in needle
                    work_needle = needle.slice_from(overlaped);
                } else {
                    // we may have partial match before this point,
                    // so we need to reset work_needle and start again
                    work_needle = needle;
                }
            }
            msum += n.size();
        }

        None
    }

    /// Copy size bytes from chain starting from specified offset.
    /// # Example:
    /// ```
    /// use chainbuf::Chain;
    /// let mut chain = Chain::new();
    /// chain.append_bytes("helloworld".as_bytes());
    /// assert_eq!(chain.copy_bytes_from(2, 2), "ll".as_bytes().to_vec());
    /// ```
    pub fn copy_bytes_from(&self, offs: uint, size: uint) -> Vec<u8> {
        if offs > self.len() {
            return Vec::new();
        }
        let buf_size = cmp::min(size, self.len() - offs);
        let mut buf = Vec::with_capacity(buf_size);
        let mut msize = buf_size;
        // Cannot fail: offs < self.len()
        let node_info = self.node_at_pos(offs).unwrap();
        let mut node:Option<&Node> = Some(node_info.node);
        let mut moffs = node_info.offset;
        let mut nodes_it = self.head.iter().skip(node_info.pos + 1);
        while node.is_some() && (msize > 0) {
            let tocopy = cmp::min(node.unwrap().size() - moffs, size);
            let d = node.unwrap().get_data_from(moffs, tocopy);
            buf.extend(d.iter().map(|x| x.clone()));
            msize -= d.len();
            moffs = 0;
            node = nodes_it.next();
        }

        return buf;
    }

    // XXX: private
    // XXX: horrible code duplication with only difference in `mut` :(
    fn node_at_pos_mut<'a>(&'a mut self, pos: uint) -> Option<NodeAtPosInfoMut> {
        if (pos << 1) > self.len() {
            // Find from tail
            let mut toff = self.len(); // tail offset
            for (i, n) in self.head.iter_mut().rev().enumerate() {
                let nsize = n.size();
                if (toff - pos) <= nsize {
                    return Some(NodeAtPosInfoMut {
                        node: n,
                        pos: i,
                        offset: (nsize - (toff - pos))
                    })
                }
                toff -= nsize;
            }
        } else {
            // Find from begining
            let mut hoff = 0; // head offset
            for (i, n) in self.head.iter_mut().enumerate() {
                let nsize = n.size();
                if (pos - hoff) < nsize {
                    return Some(NodeAtPosInfoMut {
                        node: n,
                        pos: i,
                        offset: pos - hoff
                    })
                }
                hoff += nsize;
            }
        }
        None
    }

    fn node_at_pos<'a>(&'a self, pos: uint) -> Option<NodeAtPosInfo> {
        if (pos << 1) > self.len() {
            // Find from tail
            let mut toff = self.len(); // tail offset
            for (i, n) in self.head.iter().rev().enumerate() {
                let nsize = n.size();
                if (toff - pos) <= nsize {
                    return Some(NodeAtPosInfo {
                        node: n,
                        pos: i,
                        offset: (nsize - (toff - pos))
                    })
                }
                toff -= nsize;
            }
        } else {
            // Find from begining
            let mut hoff = 0; // head offset
            for (i, n) in self.head.iter().enumerate() {
                let nsize = n.size();
                if (pos - hoff) < nsize {
                    return Some(NodeAtPosInfo {
                        node: n,
                        pos: i,
                        offset: pos - hoff
                    })
                }
                hoff += nsize;
            }
        }
        None
    }


    fn add_node_tail(&mut self, node: Node) {
        self.length += node.size();
        self.head.push(node);
    }

    fn add_node_head(&mut self, node: Node) {
        self.length += node.size();
        self.head.push_front(node);
    }
}

/// Chains are equal if they content are equal.
/// Memory layout is not important.
impl PartialEq for Chain {
    fn eq(&self, other: &Chain) -> bool {
        if self.len() != other.len() {
            return false;
        }

        let mut it1 = self.head.iter();
        let mut it2 = other.head.iter();
        let mut n1 = it1.next();
        let mut n2 = it2.next();
        let mut ofs1 = 0;
        let mut ofs2 = 0;
        while n1.is_some() && n2.is_some() {
            let node1 = n1.unwrap();
            let node2 = n2.unwrap();
            let nit1 = node1.get_data_from(ofs1, node1.size() - ofs1).iter();
            let nit2 = node2.get_data_from(ofs2, node2.size() - ofs2).iter();
            for (d1, d2) in nit1.zip(nit2) {
                if d1 != d2 {
                    return false;
                }
                ofs1 += 1;
                ofs2 += 1;
            }
            if ofs1 >= node1.size() {
                n1 = it1.next();
                ofs1 = 0;
            }
            if ofs2 >= node2.size() {
                n2 = it2.next();
                ofs2 = 0;
            }
        }
        // We have size check before the loop and the loop simultaneously
        // consumes bytes from both chains, so here we have identical chains
        // (with possibly different layouts).
        return true;
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
    #[inline]
    fn new(dh: Rc<DataHolder>) -> Node {
        Node {
            dh: dh,
            start: 0,
            end: 0
        }
    }

    #[inline]
    fn size(&self) -> uint {
        self.end - self.start
    }

    #[inline]
    fn room(&self) -> uint {
        self.dh.size - self.end
    }

    #[inline]
    fn get_data_from_start(&self, size:uint) -> &[u8] {
        self.dh.get_data(self.start, size)
    }

    #[inline]
    fn get_data_from(&self, offs: uint, size: uint) -> &[u8] {
        self.dh.get_data(self.start + offs, size)
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
        let len = src.len();
        let sd = self.data.as_mut_slice().slice_mut(dst_offs,
                                                    dst_offs + len);
        if len > sd.len() {
            fail!("copy_data_from: source larger than destination");
        }
        bytes::copy_memory(sd, src);
    }

    #[inline]
    fn get_data(&self, offset: uint, size: uint) -> &[u8] {
        self.data.slice(offset, offset + size)
    }

    #[inline]
    fn get_data_mut(&mut self, offset: uint, size: uint) -> &mut [u8] {
        self.data.as_mut_slice().slice_mut(offset, offset + size)
    }
}
