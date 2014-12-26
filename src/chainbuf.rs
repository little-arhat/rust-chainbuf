use std::cmp;
use std::str;
use std::str::Utf8Error;
use std::mem;

use std::rc::{mod, Rc};

use collections::dlist::DList;
use collections::slice::bytes;

// Put these in other module and extend Chain
#[cfg(feature="nix")] use nix::fcntl as nf;
#[cfg(feature="nix")] use nix::errno::{SysResult};
#[cfg(feature="nix")] use nix::unistd::{writev, Iovec, close};
#[cfg(feature="nix")] use nix::sys::stat;
#[cfg(feature="nix")] use nix::sys::mman;
#[cfg(feature="nix")] use std::path::Path;
#[cfg(feature="nix")] use std::io::FilePermission;
#[cfg(feature="nix")] use std::num::from_i64;
#[cfg(feature="nix")] use libc;
#[cfg(feature="nix")] use std::raw::Slice as RawSlice;


pub static CHB_MIN_SIZE:uint = 32u;

/// Move at most n items from the front of src deque to thes back of
/// dst deque.
// XXX: if we had access to DList impl, we could do this more effective
fn move_n<T>(src: &mut DList<T>, dst: &mut DList<T>, n: uint) {
    let mut nc = n;
    while nc > 0 {
        if let Some(el) = src.pop_front() {
            dst.push_back(el);
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

fn find_overlap<U:Eq, T:Iterator<U> + Clone>(large: T, short: T) -> uint {
    let mut haystack_it = large.clone();
    let mut needle_it = short.clone();
    let mut matched = 0u;
    let mut current_needle = needle_it.clone();
    loop {
        if let Some(b) = current_needle.next() {
            if let Some(h) = haystack_it.next() {
                if b == h {
                    // save position of iter for backtracking
                    needle_it = current_needle.clone();
                    current_needle = current_needle.clone();
                    matched += 1;
                } else {
                    // match failed, if we have previous matches,
                    // restore iter
                    if matched > 0 {
                        current_needle = needle_it.clone();
                    }
                    // restore haystack iter
                    haystack_it = large.clone();
                    matched = 0;
                }
            } else {
                // haystack exhausted
                haystack_it = large.clone();
            }
        } else {
            break;
        }
    }
    return matched;
}


/// Chained buffer of bytes.
/// # Example
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
pub struct Chain<'src> {
    head: DList<Node<'src>>,
    length: uint
}

struct NodeAtPosInfoMut<'a, 'src:'a> {
    node: &'a mut Node<'src>, // link to node
    pos: uint, // position of node in chain
    offset: uint // offset inside node
}

struct NodeAtPosInfo<'a, 'src:'a> {
    node: &'a Node<'src>, // link to node
    pos: uint, // position of node in chain
    offset: uint // offset inside node
}


impl<'src> Chain<'src> {
    /// Creates new, empty chainbuf.
    /// Chainbuf will not allocate any nodes until something are
    /// pushed onto it.
    /// # Example
    /// ```
    /// use chainbuf::Chain;
    /// let mut chain = Chain::new();
    /// ```
    pub fn new() -> Chain<'src> {
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
    pub fn from_foreign(src: Chain<'src>) -> Chain<'src> {
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
    /// # Example
    /// ```
    /// use chainbuf::Chain;
    /// let mut chain = Chain::new();
    /// chain.append_bytes("helloworld".as_bytes());
    /// println!("{}", chain.len()); // should print 10
    /// ```
    pub fn append_bytes(&mut self, data: &[u8]) {
        let size = data.len();
        // XXX: Damn, https://github.com/rust-lang/rust/issues/6393
        let should_create = match self.head.back() {
            Some(nd) => {
                (nd.room() < size) || nd.holds_readonly()
            }
            None => {
                true
            }
        };
        // We either not the only owner of DH or don't have enough room
        if should_create {
            let nsize = if size < CHB_MIN_SIZE { size << 1 } else { size };
            let node = Node::with_size(nsize);
            self.add_node_tail(node);
        }
        // infailable: added node above
        let node = self.head.back_mut().unwrap();
        // XXX: Damn, https://github.com/rust-lang/rust/issues/6268
        // XXX: we need additional var and scope only to fight borrow checker
        {
            let node_end = node.end;
            // we should be sole owner of data holder inside node here
            let dh = node.dh.holder_mut().unwrap();
            dh.fill_from(node_end, data);
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
                size > nd.start || nd.holds_readonly()
            }
            None => {
                true
            }
        };
        if should_create {
            let nsize = if size < CHB_MIN_SIZE { size << 1 } else { size };
            let mut node = Node::with_size(nsize);
            let r = node.room();
            node.start = r;
            node.end = r;
            self.add_node_head(node);
        }
        // See comments in `append_bytes`
        let node = self.head.front_mut().unwrap();
        {
            let node_start = node.start;
            let dh = node.dh.holder_mut().unwrap();
            dh.fill_from(node_start - size, data);
        }
        node.start -= size;
        self.length += size;
    }

    /// Appends unowned *slice* to the chain without copy.
    /// Chain lifetime became bound to one of the slice.
    /// Example
    /// ```
    /// use chainbuf::Chain;
    /// let mut chain = Chain::new();
    /// let s = "HelloWorld";
    /// chain.append_slice(s.as_bytes());
    /// println!("{}", chain.len()); // should print 10
    /// ```
    pub fn append_slice(&mut self, data: &'src [u8]) {
        let mut node = Node::with_data_holder(MemoryWrapper::new(data));
        node.end = node.room();
        self.add_node_tail(node);
    }

    /// Returns *size* bytes from the beginning of chain or None,
    /// if chain does not have enough data.
    /// # Note
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
    pub fn pullup(&self, size: uint) -> Option<&[u8]> {
        // This method logically immutable, so it's nice to have &self
        // in method signature.
        // Idiomatic Rust way to implement such methods is to use RefCell.
        // But we only need such feature in this method, and using RefCell
        // to access nodelist (self.head) in all other methods looks like
        // an overkill. And we'll also lose statically checked lifetimes for
        // main field of Chain datastructure.
        // It seems implementing this method with `unsafe` and `pullup` is
        // far better than using RefCell everywhere.
        if size == 0 || size > self.len() {
            return None
        }
        // could not fail, because self.size() > 0 => has node
        if self.head.front().unwrap().size() >= size {
            let node = self.head.front().unwrap();
            return Some(node.get_data_from_start(size));
        }
        let mut newn = Node::with_size(size);

        let mut_self: &mut Chain;
        unsafe { mut_self = mem::transmute(self); }
        // XXX: we need this scope to be able to move newn inside our list
        {
            let mut msize = size;
            while msize > 0 {
                {
                    let node = mut_self.head.front_mut().unwrap();
                    let csize = cmp::min(node.size(), msize);
                    // XXX: we need this scope only to beat borrow checker
                    {
                        let node_end = newn.end;
                        // we just created new data holder, so we have unique ownership
                        let dh = newn.dh.holder_mut().unwrap();
                        dh.fill_from(node_end,
                                     node.get_data_from_start(csize));
                    }
                    newn.end += csize;

                    if node.size() > msize {
                        node.start += msize;
                        mut_self.length -= msize;
                        break
                    }
                }
                // infailable
                let n = mut_self.head.pop_front().unwrap();
                mut_self.length -= n.size();
                msize -= n.size();
            }
        }
        mut_self.add_node_head(newn);
        // Now first node.size >= size, so we recurse
        return mut_self.pullup(size)
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
    pub fn pullup_from(&'src self, offs: uint, size: uint) -> Option<&[u8]> {
        if (offs >= self.len()) || (size == 0) {
            return None;
        }
        // Fast path: check whether node at this position have all requested
        // data:
        // We've done sanity check, so can safely unwrap this:
        let node_info = self.node_at_pos(offs).unwrap();
        if size <= node_info.node.size() - node_info.offset {
            return Some(node_info.node.get_data_from(node_info.offset,
                                                     size));
        }
        // If it's not the case, we need to rebuild our chain to provide
        // contigious region of memory.
        // We need mutable reference to self for this, so once again use
        // std::mem::transmute:
        let mut tmp = Chain::new();
        {
            let mut_self: &mut Chain;
            unsafe { mut_self = mem::transmute(self); };
            tmp.move_from(mut_self, offs);
        }
        // Run pullup to be sure, that we have dataholder that contains
        // requested number of bytes in contigious memory
        let _ = self.pullup(size);
        {
            let mut_self: &mut Chain;
            unsafe { mut_self = mem::transmute(self); };
            tmp.move_all_from(mut_self);
            // Here we have emtpy mut_self
            mut_self.concat(tmp);
        }

        // Now we can be sure that requested data fits inside one node, so
        // we recurse into itself to take Fast Path.
        return self.pullup_from(offs, size);
    }

    /// Finds first occurence of *needle* inside chain and returns data
    /// from the beginning of chain to the end of found sequence.
    /// Returns None if nothing was found.
    /// # Example
    /// ```
    /// use chainbuf::Chain;
    /// let mut chain = Chain::new();
    /// chain.append_bytes("helloworld".as_bytes());
    /// let res = chain.pullup_to("wor".as_bytes());
    /// assert_eq!(res.unwrap(), "hellowor".as_bytes());
    /// ```
    pub fn pullup_to(&self, needle: &[u8]) -> Option<&[u8]> {
        match self.find(needle) {
            Some(offset) => {
                self.pullup(offset + needle.len())
            }
            None => { return None; }
        }
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
    pub fn pullup_all(&self) -> Option<&[u8]> {
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
    /// assert!(res.some().is_ok());
    /// assert_eq!(res.unwrap().ok().unwrap(), "helloworld");
    /// ```
    pub fn to_utf8_str(&self) -> Option<Result<&str, Utf8Error>> {
        match self.pullup_all() {
            Some(bytes) => { Some(str::from_utf8(bytes)) }
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
    pub fn concat(&mut self, src: Chain<'src>) {
        self.length += src.length;
        self.head.append(src.head);
        // No need to cleanup `src`, because it has moved and cannot be used
    }

    /// Discards all data in chain, deletes all nodes and set length to 0.
    /// # Example
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
    pub fn append(&mut self, src: &Chain<'src>) {
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
    pub fn move_from<'a>(&mut self, src: &'a mut Chain<'src>, size: uint) -> uint {
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
    pub fn move_all_from(&mut self, src: &mut Chain<'src>) {
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
                (nd.room() < size) || nd.holds_readonly()
            }
            None => {
                true
            }
        };
        // We either not the only owner of DH or don't have enough room
        if should_create {
            let nsize = if size < CHB_MIN_SIZE { size << 1 } else { size };
            let node = Node::with_size(nsize);
            self.add_node_tail(node);
        }
        // infailable: have node, or have added it above
        let node = self.head.back_mut().unwrap();
        let dh = node.dh.holder_mut().unwrap();
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
                if node.size() > msize {
                    node.start += msize;
                    self.length -= msize;
                    break;
                }
            }
            // infailable
            let node = self.head.pop_front().unwrap();
            self.length -= node.size();
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
                    // Otherwase, we'srce searching for suffix of node_data
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
    /// # Example
    /// ```
    /// use chainbuf::Chain;
    /// let mut chain = Chain::new();
    /// chain.append_bytes("helloworld".as_bytes());
    /// assert_eq!(chain.copy_bytes_from(2, 2), "ll".as_bytes().to_vec());
    /// ```
    pub fn copy_bytes_from(&'src self, offs: uint, size: uint) -> Vec<u8> {
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

    /// Writes content of chain to specified file descriptor *fd*. Amount of
    /// successfully written bytes are then drained out of the chain and
    /// returned.
    /// Optional *nodes* and *size* allow to control amount of nodes that
    /// will be written.
    /// *nodes* specifies exact number of nodes to be written.
    /// *size* specifies minimum number of bytes that should be present in
    /// nodes.
    /// # Note
    /// It uses writev underneath, each node's content will go in corresponding
    /// iovec struct in array of iovecs.
    /// # Example
    /// ```
    /// extern crate nix;
    /// use chainbuf::Chain;
    /// use nix::unistd::{pipe, close, read};
    /// fn main() {
    ///     let (reader, writer) = pipe().unwrap();
    ///     let mut chain = Chain::new();
    ///     let d = "HelloWorld".as_bytes();
    ///     chain.append_bytes(d);
    ///     let written = chain.write_to_fd(writer, None, None).ok().unwrap();
    ///     close(writer);
    ///     let mut read_buf = Vec::from_elem(written, 0u8);
    ///     let read = read(reader, read_buf.as_mut_slice()).ok().unwrap();
    ///     assert_eq!(read, written);
    ///     assert_eq!(read_buf.as_slice(), d);
    ///     close(reader);
    /// }
    /// ```
    #[cfg(feature = "nix")]
    pub fn write_to_fd(&mut self, fd: nf::Fd, size:Option<uint>, nodes:Option<uint>) -> SysResult<uint> {
        let max_size = if size.is_some() { size.unwrap() } else { self.len() };
        let max_nodes = if nodes.is_some() { nodes.unwrap() } else { self.head.len() };
        // XXX: want to allocate this on stack, though
        let mut v = Vec::with_capacity(max_nodes);
        let mut towrite = 0;
        for n in self.head.iter().take(max_nodes) {
            let ns = n.size();
            v.push(Iovec::from_slice(n.get_data_from_start(ns)));
            towrite += ns;
            if towrite >= max_size {
                break;
            }
        }

        let res = writev(fd, v.as_slice());
        if res.is_ok() {
            self.drain(res.ok().unwrap());
        }
        return res
    }

    /// Appends file on *path* to chainbuf by memory mapping it.
    /// File will be closed and unmapped when node freshly created
    /// read-only node will be dropped.
    /// # Example:
    /// ```ignore
    /// use chainbuf::Chain;
    /// let path = std::path::Path::new("/tmp/path");
    /// let mut chain = Chain::new();
    /// chain.append_file(path);
    /// println!("{}", chain.len());
    /// assert!(chain.len() > 0);
    /// ```
    #[cfg(feature = "nix")]
    pub fn append_file(&mut self, path: &Path) -> SysResult<()> {
        let fd = try!(nf::open(path, nf::O_RDONLY, FilePermission::empty()));
        let fdst = try!(stat::fstat(fd));
        // XXX: fstat's st_size is signed, but in practice it shouldn't be
        let size:uint = from_i64(fdst.st_size).unwrap();
        let dh = try!(MmappedFile::new(fd, size));
        let mut node = Node::with_data_holder(dh);
        node.end = node.room();
        self.add_node_tail(node);
        return Ok(());
    }

    // XXX: private
    // XXX: horrible code duplication with only difference in `mut` :(
    fn node_at_pos_mut<'a>(&'a mut self, pos: uint) -> Option<NodeAtPosInfoMut<'a, 'src>> {
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

    fn node_at_pos<'a>(&'a self, pos: uint) -> Option<NodeAtPosInfo<'a, 'src>> {
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


    fn add_node_tail(&mut self, node: Node<'src>) {
        self.length += node.size();
        self.head.push_back(node);
    }

    fn add_node_head(&mut self, node: Node<'src>) {
        self.length += node.size();
        self.head.push_front(node);
    }
}

/// Chains are considered equal iff they have same content inside.
/// Memory layout is not important.
impl<'src> PartialEq for Chain<'src> {
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
struct Node<'src> {
    dh: DataHolder<'src>,
    start: uint,
    end: uint
}

impl<'src> Node<'src> {
    #[inline]
    /// Creates new node with MemoryBuffer of *size* bytes as dataholder
    fn with_size(size: uint) -> Node<'src> {
        Node::with_data_holder(MemoryBuffer::new(size))
    }

    #[inline]
    fn with_data_holder(dh: DataHolder<'src>) -> Node<'src> {
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
        self.dh.holder().size() - self.end
    }

    #[inline]
    fn holds_readonly(&self) -> bool {
        self.dh.is_readonly()
    }

    #[inline]
    fn get_data_from_start(&self, size:uint) -> &[u8] {
        self.dh.holder().get_data(self.start, size)
    }

    #[inline]
    fn get_data_from(&self, offs: uint, size: uint) -> &[u8] {
        self.dh.holder().get_data(self.start + offs, size)
    }
}

impl<'src> Clone for Node<'src> {
    #[inline]
    fn clone(&self) -> Node<'src> {
        let mut newn = Node::with_data_holder(self.dh.clone());
        newn.start = self.start;
        newn.end = self.end;
        newn
    }
}

/// Trait representing immutable data holders: mmap, mem wrapper, enc.
trait ImmutableDataHolder {
    /// Returns *size* bytes from dataholder starting from *offset*.
    fn get_data(&self, offset: uint, size: uint) -> &[u8];
    /// Return size of dataholder.
    fn size(&self) -> uint;
}

/// Trait representing _possible_ mutable data holders.
trait MutableDataHolder : ImmutableDataHolder {
    /// Fills buffer from offset *dst_offs* by copying data from supplied
    /// buffer *src*.
    fn fill_from(&mut self, dst_offs: uint, src: &[u8]);

    /// Returns mutable slice pointing to *size* bytes inside dataholder
    /// starting from *offset*.
    fn get_data_mut(&mut self, offset: uint, size: uint) -> &mut [u8];

    /// Upcast &MutableDataHolder to &ImmutableDataHolder
    // XXX: rust doesn't support upcasting to supertrait yet
    // https://github.com/rust-lang/rust/issues/5665
    fn as_immut<'a>(&'a self) -> &'a ImmutableDataHolder { self as &ImmutableDataHolder }
}

/// DataHolder type.
enum DataHolder<'src> {
    // XXX: Rc over Box is double indirection, because Rc internally is a Box
    // itself. However, Rc is not implemented for DST, so we can not put Trait
    // inside. And we can not implement our version of Rc because of this bug:
    // https://github.com/rust-lang/rust/issues/17959
    Mutable(Rc<Box<MutableDataHolder + 'src>>),
    Immutable(Rc<Box<ImmutableDataHolder + 'src>>)
}

impl<'src> DataHolder<'src> {
    #[inline]
    fn holder_mut(&mut self) -> Option<&mut MutableDataHolder> {
        match self {
            &DataHolder::Mutable(ref mut rcbdh) => {
                if let Some(bdh) = rc::get_mut(rcbdh) {
                    Some(&mut **bdh)
                } else {
                    None
                }
            }
            &DataHolder::Immutable(_) => { None }
        }
    }

    #[inline]
    fn holder(&self) -> &ImmutableDataHolder {
        match self {
            &DataHolder::Mutable(ref mbdh) => {
                (&***mbdh).as_immut()
            }
            &DataHolder::Immutable(ref imbdh) => {
                & ***imbdh
            }
        }
    }

    #[inline]
    fn is_readonly(&self) -> bool {
        match self {
            &DataHolder::Mutable(ref rcbdh) => {
                !rc::is_unique(rcbdh)
            }
            &DataHolder::Immutable(_) => { true }
        }
    }
}

impl<'src> Clone for DataHolder<'src> {
    #[inline]
    fn clone(&self) -> DataHolder<'src> {
        match self {
            &DataHolder::Mutable(ref rcbdh) => { DataHolder::Mutable(rcbdh.clone()) }
            &DataHolder::Immutable(ref rcbdh) => { DataHolder::Immutable(rcbdh.clone()) }
        }
    }
}

/// Refcounted data holder
// TODO: implement other storages: shmem
struct MemoryBuffer{
    size: uint,
    data: Vec<u8>
}

impl MemoryBuffer {
    #[inline]
    fn new<'src>(size: uint) -> DataHolder<'src> {
        DataHolder::Mutable(Rc::new(box MemoryBuffer {
            size: size,
            data: Vec::from_elem(size, 0)
        } as Box<MutableDataHolder>))
    }
}

impl ImmutableDataHolder for MemoryBuffer {
    #[inline]
    fn get_data(&self, offset: uint, size: uint) -> &[u8] {
        self.data.slice(offset, offset + size)
    }

    #[inline]
    fn size(&self) -> uint {
        self.size
    }
}

impl MutableDataHolder for MemoryBuffer {
    #[inline]
    fn fill_from(&mut self, dst_offs: uint, src: &[u8]) {
        let len = src.len();
        let sd = self.data.as_mut_slice().slice_mut(dst_offs,
                                                    dst_offs + len);
        if len > sd.len() {
            panic!("copy_data_from: source larger than destination");
        }
        bytes::copy_memory(sd, src);
    }

    #[inline]
    fn get_data_mut(&mut self, offset: uint, size: uint) -> &mut [u8] {
        self.data.as_mut_slice().slice_mut(offset, offset + size)
    }
}


/// Dataholder as wrapper over some unowned slice.
struct MemoryWrapper<'a> {
    data: &'a [u8]
}

impl <'a>MemoryWrapper<'a> {
    fn new<'src>(data: &'src [u8]) -> DataHolder<'src> {
        DataHolder::Immutable(Rc::new(box MemoryWrapper{
            data: data
        } as Box<ImmutableDataHolder>))
    }
}

impl<'a> ImmutableDataHolder for MemoryWrapper<'a> {
    #[inline]
    fn get_data(&self, offset: uint, size: uint) -> &[u8] {
        self.data.slice(offset, offset + size)
    }

    #[inline]
    fn size(&self) -> uint {
        self.data.len()
    }
}

/// Dataholder as wrapper over mmaped file.
#[cfg(feature="nix")]
struct MmappedFile<'a> {
    size: uint,
    fd: nf::Fd,
    addr: *const u8
}

impl<'a> MmappedFile<'a> {
    fn new<'src>(fd:nf::Fd, size:uint) -> SysResult<DataHolder<'src>> {
        let addr = try!(mman::mmap(0 as *mut libc::c_void,
                                   size as u64, mman::PROT_READ,
                                   mman::MAP_SHARED, fd, 0));

        let dh = DataHolder::Immutable(Rc::new(box MmappedFile {
            size: size,
            fd: fd,
            addr: addr as *const u8
        } as Box<ImmutableDataHolder>));
        Ok(dh)
    }
}

#[unsafe_destructor]
impl<'a> Drop for MmappedFile<'a> {
    fn drop(&mut self) {
        let munmap_res = mman::munmap(self.addr as *mut libc::c_void,
                                      self.size as libc::size_t);
        let close_res = close(self.fd);
        assert!(munmap_res.is_ok() && close_res.is_ok());
    }
}

impl<'a> ImmutableDataHolder for MmappedFile<'a> {
    #[inline]
    fn get_data(&self, offset: uint, size: uint) -> &[u8] {
        unsafe {
            mem::transmute(RawSlice{
                data:self.addr.offset(offset as int),
                len: size
            })
        }
    }

    #[inline]
    fn size(&self) -> uint {
        self.size
    }
}
