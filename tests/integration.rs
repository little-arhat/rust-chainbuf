extern crate chainbuf;
#[cfg(feature="nix")] extern crate nix;

#[cfg(test)]
mod test {
    #[cfg(feature="nix")]
    mod test_writev {
        use chainbuf::Chain;
        use nix::unistd::{pipe, close, read};
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
            let read_res = read(reader, read_buf.as_mut_slice());
            assert!(read_res.is_ok());
            let read = read_res.ok().unwrap() as uint;
            // Check we have read as much as we written
            assert_eq!(read, written);
            assert_eq!(to_write.as_slice(), read_buf.as_slice());
            let _ = close(writer);
            let _ = close(reader);
        }
    }

    #[cfg(feature="nix")]
    mod test_append_file {
        use chainbuf::Chain;
        use nix::unistd::{close, write};
        use nix::fcntl as nf;
        use std::rand::{task_rng, Rng};
        use std::io::{TempDir, USER_FILE};

        #[test]
        fn test_append_flie() {
            let s:String = task_rng().gen_ascii_chars().take(1024).collect();
            let v = s.into_bytes();
            let tmpd_res = TempDir::new("chain-test");
            assert!(tmpd_res.is_ok());
            let tmpd = tmpd_res.ok().unwrap();
            let mut p = tmpd.path().clone();
            p.push("mmaped_file.map");
            let open_res = nf::open(&p,
                                    nf::O_CREAT | nf::O_RDWR | nf::O_TRUNC,
                                    USER_FILE);
            assert!(open_res.is_ok());
            let fd = open_res.ok().unwrap();
            let write_res = write(fd, v.as_slice());
            assert!(write_res.is_ok());
            let close_res = close(fd);
            assert!(close_res.is_ok());
            let written = write_res.ok().unwrap();
            let mut chain = Chain::new();
            let apfile_res = chain.append_file(&p);
            assert!(apfile_res.is_ok());
            assert_eq!(chain.len(), written);
            let pulled = chain.pullup(written);
            assert!(pulled.is_some());
            let data = pulled.unwrap();
            assert_eq!(data, v.as_slice());
        }
    }

}
