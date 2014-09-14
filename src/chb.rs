extern crate collections;

use collections::dlist::DList;
use collections::Deque;

static CHB_MIN_SIZE:uint = 32u;


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
        data: Vec::with_capacity(size)
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

fn chb_create_node_tail(chain: &mut ChbChain, size: uint) {
    let nsize = if size < CHB_MIN_SIZE {
        size << 1
    } else {
        size
    };
    let node = chb_node_new(chb_dh_new(nsize)); // Box<ChbNode>
    chb_add_node_tail(chain, node);
}

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

fn chb_append_bytes(dst: &mut ChbChain, data: &[u8]) {
    let size = data.len();
    // Check is READONLY
    let should_create = match dst.head.back() {
        Some(nd) => {
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
    node.dh.data.push_all(data);
    node.end += size;
    dst.length += size;
}

fn chb_prepend_bytes(dst: &mut ChbChain, data: &[u8]) {
    let size = data.len();

}

fn main() {
    let mut chain = chb_new();
    chb_append_bytes(&mut chain, "fwdhwdfhpudqbpudwfpldqfpludhqpflolololol".as_bytes());
}
