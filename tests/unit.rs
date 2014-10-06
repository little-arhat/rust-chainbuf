
extern crate chainbuf;


#[cfg(test)]
mod test {
    use chainbuf::{CHB_MIN_SIZE, Chain};
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
    fn test_pullup_returns_none_on_empty_chain() {
        let chain = Chain::new();
        assert!(chain.pullup(1).is_none());
    }

    #[test]
    fn test_pullup_returns_what_has_been_appended() {
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
    fn test_pullup_all_returns_all_data() {
        let mut chain = Chain::new();
        let s = "helloworld".as_bytes();
        chain.append_bytes(s);
        let res = chain.pullup_all();
        assert!(res.is_some());
        assert_eq!(res.unwrap(), s);
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

    #[test]
    fn test_move_from_moves_data() {
        let mut chain1 = Chain::new();
        let mut chain2 = Chain::new();
        let s = "HelloWorld";
        let b = s.as_bytes();
        let lb = b.len();
        let hlb = lb / 2;
        chain1.append_bytes(b);
        chain2.append_bytes(b);
        chain1.move_from(&mut chain2, hlb);
        assert_eq!(chain1.len(), lb + hlb);
        assert_eq!(chain2.len(), hlb);
        {
            let mut ss = String::from_str(s);
            ss.push_str(s.slice(0, hlb));
            let r = b.slice_from(hlb);
            let r1 = chain1.pullup(lb + hlb);
            let r2 = chain2.pullup(hlb);
            assert!(r1.is_some());
            assert!(r2.is_some());
            assert_eq!(r1.unwrap(), ss.as_bytes());
            assert_eq!(r2.unwrap(), r);
        }
    }

    #[test]
    fn test_move_from_move_on_node_edge() {
        let mut chain1 = Chain::new();
        let mut chain2 = Chain::new();
        let s:String = task_rng().gen_ascii_chars().take(CHB_MIN_SIZE).collect();
        let sb = s.as_bytes();
        chain2.append_bytes(sb);
        chain2.append_bytes(sb);
        chain2.append_bytes(sb);
        chain2.append_bytes(sb);
        chain1.move_from(&mut chain2, CHB_MIN_SIZE * 2);
        assert_eq!(chain1.len(), chain2.len());
    }

    #[test]
    fn test_move_from_returns_number_of_bytes_moved() {
        let mut chain1 = Chain::new();
        let mut chain2 = Chain::new();
        chain2.append_bytes("helloworld".as_bytes());
        let orig_size = chain2.len();
        let moved = chain1.move_from(&mut chain2, 3);
        let new_size = chain2.len();
        assert_eq!(moved, 3);
        assert_eq!(orig_size - moved, chain2.len());
        let moved_some_more = chain1.move_from(&mut chain2, orig_size);
        assert_eq!(moved_some_more, new_size);
        assert_eq!(chain2.len(), 0);
    }

    #[test]
    fn test_reserve_returns_buffer_of_requested_size() {
        let mut chain = Chain::new();
        let buf = chain.reserve(10);
        assert_eq!(buf.len(), 10);
    }

    #[test]
    fn test_reserve_returns_free_buffer() {
        let mut chain = Chain::new();
        chain.append_bytes("helloworld".as_bytes());
        let buf = chain.reserve(10);
        let pat = Vec::from_elem(10, 0u8);
        assert_eq!(buf.as_slice(), pat.as_slice());
    }

    #[test]
    fn test_reserve_and_written_modifies_chain() {
        let mut chain = Chain::new();
        let s = "helloworld".as_bytes();
        let sl = s.len();
        {
            let buf = chain.reserve(10);
            for (i, c) in s.iter().enumerate() {
                buf[i] = *c as u8;
            }
        }
        chain.written(sl);
        assert_eq!(chain.len(), sl);
        assert_eq!(chain.pullup(sl).unwrap(), s);
    }

    #[test]
    fn test_drain_changes_chain_length() {
        let mut chain = Chain::new();
        let s = "helloworld".as_bytes();
        let hsl = s.len() / 2;
        chain.append_bytes(s);
        let was_l = chain.len();
        chain.drain(hsl);
        let new_l = chain.len();
        assert!(new_l < was_l);
        assert_eq!(new_l, was_l - hsl);
    }

    #[test]
    fn test_to_utf8_str_returns_none_on_non_utf8() {
        let mut chain = Chain::new();
        let b = [0xf0_u8, 0xff_u8, 0xff_u8, 0x10_u8];
        chain.append_bytes(b);
        let res = chain.to_utf8_str();
        assert!(res.is_none());
    }

    #[test]
    fn test_to_utf8_returns_correct_string() {
        let mut chain = Chain::new();
        let s:String = task_rng().gen_ascii_chars().take(CHB_MIN_SIZE * 4).collect();
        chain.append_bytes(s.as_bytes());
        let res = chain.to_utf8_str();
        assert!(res.is_some());
        assert_eq!(res.unwrap(), s.as_slice());
    }

    #[test]
    fn test_find_returns_none_on_empty_chain() {
        let chain = Chain::new();
        let res = chain.find("helloworld".as_bytes());
        assert!(res.is_none());
    }

    #[test]
    fn test_find_returns_zero_on_empty_needle() {
        let mut chain = Chain::new();
        chain.append_bytes("helloworld".as_bytes());
        let res = chain.find("".as_bytes());
        assert!(res.is_some());
        assert_eq!(res.unwrap(), 0);
    }

    #[test]
    fn test_find_returns_none_if_not_found() {
        let mut chain = Chain::new();
        let needle = [1u8, 2u8, 3u8];
        let one_seq = 128u;
        for _ in range(0u, 20) {
            let s:String = task_rng().gen_ascii_chars().take(one_seq).collect();
            let b = s.as_bytes();
            chain.append_bytes(b);
        }
        let res = chain.find(needle);
        assert!(res.is_none());
    }

    #[test]
    fn test_find_returns_correct_offset() {
        let mut chain = Chain::new();
        let mut offs = 0;
        let needle = "the quick brown fox jumps over the lazy dog";
        for i in range(0u, 20) {
            let mut int_rng = task_rng();
            let s:String = task_rng().gen_ascii_chars().take(int_rng.gen_range(50, 100)).collect();
            let bytes = s.as_bytes();

            chain.append_bytes(bytes);
            if i <= 11 {
                offs += bytes.len();
            }
            if i == 11 {
                chain.append_bytes(needle.as_bytes());
            }
        }
        let res = chain.find(needle.as_bytes());
        assert!(res.is_some());
        assert_eq!(res.unwrap(), offs);
    }

    #[test]
    fn test_chains_with_same_content_are_equal() {
        let mut chain1 = Chain::new();
        let mut chain2 = Chain::new();
        let total = 2048u;
        let mut t = total;
        let one_seq = 128u;
        while t > 0 {
            let s:String = task_rng().gen_ascii_chars().take(one_seq).collect();
            let b = s.as_bytes();
            chain1.append_bytes(b);
            chain2.append_bytes(b);
            t -= one_seq;
        }
        assert!(chain1 == chain2);
        let res1 = chain1.pullup(total).unwrap();
        let res2 = chain2.pullup(total).unwrap();
        assert_eq!(res1, res2);
    }

    #[test]
    fn test_chains_with_different_content_are_not_equal() {
        let mut chain1 = Chain::new();
        let mut chain2 = Chain::new();
        chain1.append_bytes("hello".as_bytes());
        chain2.append_bytes("world".as_bytes());
        assert!(chain1 != chain2);
    }

    #[test]
    fn test_copy_bytes_from_returns_empty_vec_from_empty_chain() {
        let chain = Chain::new();
        let empty_vec = Vec::new();
        let res = chain.copy_bytes_from(10, 10);
        assert_eq!(res, empty_vec);
    }

    #[test]
    fn test_copy_bytes_from_returns_copies_bytes() {
        let mut chain = Chain::new();
        let mut offs = 0;
        let v = vec!["helloworld", "example", "someotherstring", "differentstring"];
        for (i, el) in v.iter().enumerate() {
            chain.append_bytes(el.as_bytes());
            if i < 2 {
                offs += el.as_bytes().len();
            }
        }
        let res = chain.copy_bytes_from(offs, v[2].len());
        assert_eq!(res.as_slice(), v[2].as_bytes());
    }

    #[test]
    fn test_copy_bytes_returns_less_than_requested_if_chain_does_not_have_data() {
        let mut chain = Chain::new();
        chain.append_bytes("helloworld".as_bytes());
        let size = chain.len();
        let res = chain.copy_bytes_from(5, size);
        assert!(res.len() < size);
        assert_eq!(res.len(), size - 5);
    }

    #[test]
    fn test_pullup_from_returns_none_on_empty_chain() {
        let chain = Chain::new();
        let res = chain.pullup_from(10, 10);
        assert!(res.is_none());
    }

    #[test]
    fn test_pullup_from_returns_data_from_correct_offset() {
        let mut chain = Chain::new();
        let mut offs = 0;
        let v = vec!["helloworld", "example", "someotherstring", "differentstring"];
        for (i, el) in v.iter().enumerate() {
            chain.append_bytes(el.as_bytes());
            if i < 2 {
                offs += el.as_bytes().len();
            }
        }
        let res = chain.pullup_from(offs, v[2].len());
        assert!(res.is_some());
        assert_eq!(res.unwrap(), v[2].as_bytes());

    }
}
