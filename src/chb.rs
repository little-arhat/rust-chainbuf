extern crate collections;

use collections::dlist::DList;
use collections::Deque;

static CHB_MIN_SIZE:uint = 32u;


fn blit<T:Clone>(src: &[T], src_ofs: uint, dst: &mut [T], dst_ofs: uint, len: uint) {
    if (src_ofs > src.len() - len) || (dst_ofs > dst.len() - len) {
        fail!("blit: invalid argument!");
    }
    let sd = dst.mut_slice(dst_ofs, dst_ofs + len);
    let ss = src.slice(src_ofs, src_ofs + len);
    let _ = sd.clone_from_slice(ss);
}


struct ChbDataHolder{
    size: uint,
    data: Vec<u8>
}

struct ChbNode {
    dh: Box<ChbDataHolder>, // можно заменить на RC
    start: uint,
    end: uint
}

struct ChbChain {
    head: DList<Box<ChbNode>>,
    length: uint
}

fn chb_dh_new(size: uint) -> Box<ChbDataHolder> {
    let dh = box ChbDataHolder {
        size: size,
        data: Vec::from_elem(size, 0)
    };
    return dh;
}

// Impl + methods
fn chb_node_new(dh: Box<ChbDataHolder>) -> Box<ChbNode> {
    let n = box ChbNode {
        dh: dh,
        start: 0,
        end: 0
    };
    // ref dh ? auto, when using RC
    return n;
}

fn chb_node_size(node: &ChbNode) -> uint {
    node.end - node.start
}

fn chb_node_room(node: &ChbNode) -> uint {
    return node.dh.size - node.end;
}

fn chb_new() -> ChbChain {
    return ChbChain{
        head: DList::new(),
        length: 0
    }
}

fn chb_add_node_tail(chain: &mut ChbChain, node: Box<ChbNode>) {
    chain.length += chb_node_size(&*node);
    chain.head.push(node);
}

fn chb_add_node_head(chain: &mut ChbChain, node: Box<ChbNode>) {
    chain.length += chb_node_size(&*node);
    chain.head.push_front(node);
}

// TODO: rename _back
fn chb_create_node_tail(chain: &mut ChbChain, size: uint) {
    let nsize = if size < CHB_MIN_SIZE {
        size << 1
    } else {
        size
    };
    let node = chb_node_new(chb_dh_new(nsize)); // Box<ChbNode>
    chb_add_node_tail(chain, node);
}

// TODO: rename _front
fn chb_create_node_head(chain: &mut ChbChain, size: uint) {
    let nsize = if size < CHB_MIN_SIZE {
        size << 1
    } else {
        size
    };
    let mut node = chb_node_new(chb_dh_new(nsize)); // Box<ChbNode>
    let r = chb_node_room(&*node);
    node.start = r;
    node.end = r;
    chb_add_node_head(chain, node);
}

// XXX: maybe DEDUP append/prepend?
// TODO: test: length, capacity, node size
fn chb_append_bytes(dst: &mut ChbChain, data: &[u8]) {
    let size = data.len();
    // XXX: Damn, https://github.com/rust-lang/rust/issues/6393
    let should_create = match dst.head.back() {
        Some(nd) => {
            // Check is READONLY
            chb_node_room(&**nd) < size
        }
        None => {
            true
        }
    };
    if should_create {
        chb_create_node_tail(dst, size);
    }
    // node could not be None here
    let node = dst.head.back_mut().unwrap();
    // XXX: Damn, https://github.com/rust-lang/rust/issues/6268
    let end = node.end;
    blit(data.as_slice(), 0,
         node.dh.data.as_mut_slice(), end,
         size);
    node.end += size;
    dst.length += size;
}

// TODO: test: length, capacity, node size
fn chb_prepend_bytes(dst: &mut ChbChain, data: &[u8]) {
    let size = data.len();
    // XXX: Damn, https://github.com/rust-lang/rust/issues/6393
    let should_create = match dst.head.front() {
        Some(nd) => {
            // Check is READONLY
            size > nd.start
        }
        None => {
            true
        }
    };
    if should_create {
        chb_create_node_head(dst, size);
    }
    // node could not be None here
    let node = dst.head.front_mut().unwrap();
    // XXX: Damn, https://github.com/rust-lang/rust/issues/6268
    let start = node.start;
    blit(data.as_slice(), 0,
         node.dh.data.as_mut_slice(), start - size,
         size);
    node.start -= size;
    dst.length += size;
}

fn main() {
    let mut chain = chb_new();
    chb_append_bytes(&mut chain, "abcdefghijklmnop".as_bytes());
    chb_prepend_bytes(&mut chain, "xxx".as_bytes());
}
