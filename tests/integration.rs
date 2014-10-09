extern crate chainbuf;
extern crate native;

#[cfg(test)]
mod test {
    use chainbuf::Chain;

    //use libc;
    use std::os;
    use native::io::FileDesc;

    use std::rand::{task_rng, Rng};

    #[test]
    fn test_write_to_fd_works() {
        // Run this test with some pipes so we don't have to mess around with
        // opening or closing files.
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
        let cl = chain.len();
        let os::Pipe { reader, writer } = unsafe { os::pipe().unwrap() };
        let mut reader = FileDesc::new(reader, true);
        let mut read_buf = Vec::from_elem(total, 0u8);
        // We're doing blocking write, so should write entire buffer.
        let write_res = chain.write_to_fd(writer, None, None) as int;
        let read_res = reader.inner_read(read_buf.as_mut_slice());
        assert!(read_res.is_ok());
        assert!(write_res > 0);
        assert_eq!(write_res as uint, cl);
        assert_eq!(read_res.ok().unwrap(), write_res as uint);
        assert_eq!(buf.as_slice(), read_buf.as_slice());

    }
}
