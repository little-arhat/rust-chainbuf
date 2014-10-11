extern crate chainbuf;
extern crate native;
#[cfg(feature="nix")] extern crate nix;

#[cfg(test)]
mod test {
    #[cfg(feature="nix")]
    mod test_writev {
        use chainbuf::Chain;
        use nix::unistd::{pipe, close};
        use native::io::FileDesc;
        use std::rand::{task_rng, Rng};

        #[test]
        fn test_write_to_fd_works() {
            // Run this test with some pipes so we don't have to mess around with
            // opening or closing files.
            let mut chain = Chain::new();

            let mut to_write = Vec::with_capacity(16 * 128);
            for _ in range(0u, 16) {
                let s:String = task_rng().gen_ascii_chars().take(128).collect();
                let b = s.as_bytes();
                chain.append_bytes(b);
                to_write.extend(b.iter().map(|x| x.clone()));
            }
            let cl = chain.len();

            let pipe_res = pipe();
            assert!(pipe_res.is_ok());
            let (reader, writer) = pipe_res.ok().unwrap();
            let mut reader = FileDesc::new(reader, true);
            // write all data
            let write_res = chain.write_to_fd(writer, None, None);
            assert!(write_res.is_ok());
            let written = write_res.ok().unwrap();
            // written all data
            assert_eq!(to_write.len(), written);
            // written all that has been stored
            assert_eq!(written, cl);
            // chain has been drained
            assert_eq!(chain.len(), 0);
            let mut read_buf = Vec::from_elem(128 * 16, 0u8);
            let read_res = reader.inner_read(read_buf.as_mut_slice());
            assert!(read_res.is_ok());
            let read = read_res.ok().unwrap() as uint;
            // Check we have read as much as we written
            assert_eq!(read, written);
            assert_eq!(to_write.as_slice(), read_buf.as_slice());
            let _ = close(writer);
        }

    }

}
