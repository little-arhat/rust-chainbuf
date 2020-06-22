#[cfg(test)]
mod integration_test {
    #[cfg(feature = "nix")]
    mod test_writev {
        use chainbuf::Chain;
        use nix::unistd::{close, pipe, read};
        use rand::{thread_rng, Rng};
        use std::iter::repeat;

        #[test]
        fn test_write_to_fd_works() {
            // Run this test with some pipes so we don't have to mess around with
            // opening or closing files.
            let mut chain = Chain::new();

            let mut to_write = Vec::with_capacity(16 * 128);
            for _ in 0usize..16 {
                let s: String = thread_rng().gen_ascii_chars().take(128).collect();
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
            let mut read_buf: Vec<u8> = repeat(0u8).take(128 * 16).collect();
            let read_res = read(reader, &mut read_buf[..]);
            assert!(read_res.is_ok());
            let read = read_res.ok().unwrap() as usize;
            // Check we have read as much as we written
            assert_eq!(read, written);
            assert_eq!(&to_write[..], &read_buf[..]);
            let _ = close(writer);
            let _ = close(reader);
        }
    }

    #[cfg(feature = "nix")]
    #[allow(deprecated)]
    mod test_append_file {
        use chainbuf::Chain;
        use nix::fcntl as nf;
        use nix::sys::stat;
        use nix::unistd::{close, write};
        use rand::{thread_rng, Rng};
        use tempdir::TempDir;

        #[test]
        fn test_append_flie() {
            let s: String = thread_rng().gen_ascii_chars().take(1024).collect();
            let v = s.into_bytes();
            let tmpd_res = TempDir::new("chain-test");
            assert!(tmpd_res.is_ok());
            let tmpd = tmpd_res.ok().unwrap();
            let mut p = tmpd.path().to_path_buf();
            p.push("mmaped_file.map");
            let user_file = stat::Mode::S_IRUSR
                | stat::Mode::S_IWUSR
                | stat::Mode::S_IRGRP
                | stat::Mode::S_IROTH;
            let open_res = nf::open(
                &p,
                nf::OFlag::O_CREAT | nf::OFlag::O_RDWR | nf::OFlag::O_TRUNC,
                user_file,
            );
            assert!(open_res.is_ok());
            let fd = open_res.ok().unwrap();
            let write_res = write(fd, &v[..]);
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
            assert_eq!(data, &v[..]);
        }
    }
}
