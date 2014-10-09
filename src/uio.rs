
use libc;


#[repr(C)]
pub struct iovec {
    iov_base: *mut libc::c_void,
    iov_len: libc::size_t,
}

impl iovec {
    #[inline]
    pub fn from_slice(buf: &[u8]) -> iovec {
        iovec{
            iov_base: buf.as_ptr() as *mut libc::c_void,
            iov_len: buf.len() as libc::size_t
        }
    }
}

mod ext {
    use super::iovec;
    use libc;
    extern {
        pub fn writev(fd: libc::c_int, iovec: *const iovec, count: libc::c_int) -> libc::ssize_t;
        pub fn readv(fd: libc::c_int, iovec: *const iovec, count: libc::c_int) -> libc::ssize_t;
    }
}

#[inline]
pub fn writev(fd: libc::c_int, iovec: &[iovec]) -> libc::ssize_t {
    unsafe {
        ext::writev(fd, iovec.as_ptr(), iovec.len() as libc::c_int)
    }
}

#[allow(dead_code)]
#[inline]
pub fn readv(fd: libc::c_int, iovec: &[iovec]) -> libc::ssize_t {
    unsafe {
        ext::readv(fd, iovec.as_ptr(), iovec.len() as libc::c_int)
    }
}
