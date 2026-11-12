// file is called peeker poker
// because it's the lingo[1] that ptrace
// uses to refer to reading/writing
// from/to a tracee's memory
//
// [1]: https://en.wikipedia.org/wiki/PEEK_and_POKE
//
#[cfg(target_pointer_width = "32")]
pub const WORD_SIZE: usize = 4;

#[cfg(target_pointer_width = "64")]
pub const WORD_SIZE: usize = 8;

use std::{ffi::c_void, io::IoSliceMut, num::NonZeroUsize};

use nix::{
    libc::{cpu_set_t, CPU_ISSET, CPU_SETSIZE},
    sys::{
        ptrace,
        uio::{process_vm_readv, RemoteIoVec},
    },
    unistd::Pid,
};

fn read_bytes_with_count<const N: usize>(addr: usize, pid: Pid) -> Option<([u8; N], usize)> {
    let base = addr;
    let remote_iov = RemoteIoVec { base, len: N };
    // TODO!
    // large array sizes might overflow
    let mut bytes_buffer = [0u8; N];
    let bytes_read = process_vm_readv(
        pid,
        &mut [IoSliceMut::new(&mut bytes_buffer)],
        &[remote_iov],
    ).ok()?;
    Some((bytes_buffer, bytes_read))
}

pub fn read_bytes<const N: usize>(addr: usize, pid: Pid) -> Option<[u8; N]> {
    read_bytes_with_count(addr, pid).map(|(bytes, _)| bytes)
}

fn read_exact_bytes<const N: usize>(addr: usize, pid: Pid) -> Option<[u8; N]> {
    let (bytes, bytes_read) = read_bytes_with_count(addr, pid)?;
    (bytes_read == N).then_some(bytes)
}

pub fn read_bytes_variable_length(base: usize, pid: Pid, len: usize) -> Option<Vec<u8>> {
    let remote_iov = RemoteIoVec { base, len };
    let mut bytes_buffer = vec![0u8; len];
    // Note, however, that these system calls
    // do not check the memory regions in the remote process
    // until just before doing the read/write.
    // Consequently, a partial read/write (see RETURN VALUE) may result
    // if one of the remote_iov elements points to an invalid memory region in the remote process.
    // No further reads/writes will be attempted beyond that point.
    //
    // Keep this in mind when attempting to read data of unknown length
    // (such as C strings that are null-terminated) from a remote process,
    // by avoiding spanning memory pages (typically 4 KiB)
    // in a single remote iovec element.
    // (Instead, split the remote read into two remote_iov elements
    // and have them merge back into a single write local_iov entry.
    // The first read entry goes up to the page boundary,
    let _ = process_vm_readv(
        pid,
        &mut [IoSliceMut::new(&mut bytes_buffer)],
        &[remote_iov],
    )
    .ok()?;
    Some(bytes_buffer)
}

pub fn read_bytes_as_struct<const N: usize, T>(addr: usize, pid: Pid) -> Option<T> {
    let vec = read_bytes::<N>(addr, pid)?;
    Some(unsafe { std::mem::transmute_copy(&vec) })
}

pub fn read_i32(address: usize, pid: Pid) -> Option<i32> {
    read_exact_bytes::<4>(address, pid).map(i32::from_ne_bytes)
}

pub fn read_usize(address: usize, pid: Pid) -> Option<usize> {
    read_exact_bytes::<{ size_of::<usize>() }>(address, pid).map(usize::from_ne_bytes)
}

pub fn read_pipe_fds(address: usize, pid: Pid) -> Option<[i32; 2]> {
    let [a, b, c, d, e, f, g, h] = read_exact_bytes::<8>(address, pid)?;
    Some([
        i32::from_ne_bytes([a, b, c, d]),
        i32::from_ne_bytes([e, f, g, h]),
    ])
}

pub fn read_bytes_until_null(address: usize, pid: Pid) -> Option<Vec<u8>> {
    let mut address = address as *mut c_void;
    let mut data = vec![];
    'read_loop: loop {
        // TODO!
        // change this to be similar to read_words_until_null below
        // i.e. if err: return collected data so far
        let word = ptrace::read(pid, address).ok()?;
        let bytes: [u8; WORD_SIZE] = i64::to_ne_bytes(word);
        for byte in bytes {
            if byte == b'\0' {
                break 'read_loop;
            }
            data.push(byte);
        }
        address = unsafe { address.byte_add(WORD_SIZE) };
    }
    Some(data)
}

// usually used to resolve array of pointers to *
pub fn read_words_until_null(address: usize, pid: Pid) -> Option<Vec<usize>> {
    let mut addr = address as *mut c_void;
    let mut data = vec![];
    'read_loop: loop {
        match ptrace::read(pid, addr) {
            Ok(word) => {
                if word == 0 {
                    break 'read_loop;
                }
                data.push(word as usize);
                addr = unsafe { addr.byte_add(WORD_SIZE) };
            }
            Err(_err) => return Some(data),
        }
    }
    Some(data)
}

pub fn read_affinity_from_child(address: usize, pid: Pid) -> Option<Vec<usize>> {
    const CPU_SET_USIZE: usize = (CPU_SETSIZE / WORD_SIZE as i32) as usize;

    let cpu_set = read_bytes_as_struct::<CPU_SET_USIZE, cpu_set_t>(address, pid)?;

    let mut vec = Vec::new();
    for cpu_number in 0..std::thread::available_parallelism()
        .map_or(1, NonZeroUsize::get) as usize
    {
        if unsafe { CPU_ISSET(cpu_number, &cpu_set) } {
            vec.push(cpu_number);
        }
    }
    Some(vec)
}

pub fn read_string_specific_length(addr: usize, pid: Pid, size: usize) -> Option<String> {
    let bytes_buffer = read_bytes_variable_length(addr, pid, size)?;
    Some(String::from_utf8_lossy(&bytes_buffer).into_owned())
}

pub fn write_bytes<const N: usize>(addr: usize, pid: Pid, data: [u64; N]) -> Result<(), ()> {
    let mut addr = addr as *mut c_void;
    for word in data {
        match ptrace::write(pid, addr, word as _) {
            Ok(_void) => {
                addr = unsafe { addr.byte_add(WORD_SIZE) };
            }
            Err(_res) => return Err(()),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{read_i32, read_pipe_fds, read_usize};
    use nix::{libc, unistd::Pid};

    fn own_pid() -> Pid {
        Pid::from_raw(unsafe { libc::getpid() })
    }

    #[test]
    fn reads_values_using_their_native_width_and_byte_order() {
        let integer = -123_456_789_i32;
        let word = usize::MAX - 0x1234;
        let pipes = [17_i32, 258_i32];
        let pid = own_pid();

        assert_eq!(read_i32(&raw const integer as usize, pid), Some(integer));
        assert_eq!(read_usize(&raw const word as usize, pid), Some(word));
        assert_eq!(read_pipe_fds(&raw const pipes as usize, pid), Some(pipes));
        assert_eq!(read_i32(0, pid), None);
        assert_eq!(read_usize(0, pid), None);
        assert_eq!(read_pipe_fds(0, pid), None);
    }

    #[test]
    fn rejects_partial_reads_at_a_page_boundary() {
        struct Mapping(*mut libc::c_void, usize);

        impl Drop for Mapping {
            fn drop(&mut self) {
                unsafe { libc::munmap(self.0, self.1) };
            }
        }

        let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
        assert!(page_size > 0);
        let page_size = page_size as usize;
        let length = page_size * 2;
        let address = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                length,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
                -1,
                0,
            )
        };
        assert_ne!(address, libc::MAP_FAILED);
        let mapping = Mapping(address, length);
        let value_address = unsafe { mapping.0.cast::<u8>().add(page_size - 4) };
        unsafe {
            value_address.copy_from_nonoverlapping(42_i32.to_ne_bytes().as_ptr(), 4);
            assert_eq!(
                libc::mprotect(
                    mapping.0.cast::<u8>().add(page_size).cast(),
                    page_size,
                    libc::PROT_NONE,
                ),
                0
            );
        }

        let pid = own_pid();
        assert_eq!(read_i32(value_address as usize, pid), Some(42));
        assert_eq!(read_usize(value_address as usize, pid), None);
        assert_eq!(read_pipe_fds(value_address as usize, pid), None);
    }
}
