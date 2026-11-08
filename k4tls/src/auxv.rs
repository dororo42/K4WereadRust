use std::io::{self, Read};

fn lookup(mut reader: impl Read, key: libc::c_ulong) -> io::Result<Option<libc::c_ulong>> {
    loop {
        let mut tag = [0; std::mem::size_of::<libc::c_ulong>()];
        let mut value = [0; std::mem::size_of::<libc::c_ulong>()];
        reader.read_exact(&mut tag)?;
        reader.read_exact(&mut value)?;
        let tag = libc::c_ulong::from_ne_bytes(tag);
        if tag == 0 {
            return Ok(None);
        }
        if tag == key {
            return Ok(Some(libc::c_ulong::from_ne_bytes(value)));
        }
    }
}

// glibc 2.12 lacks getauxval; --wrap keeps this implementation local to ring's imports.
#[cfg(all(target_arch = "arm", target_os = "linux", target_env = "gnu"))]
#[no_mangle]
extern "C" fn __wrap_getauxval(key: libc::c_ulong) -> libc::c_ulong {
    let value = std::fs::File::open("/proc/self/auxv").and_then(|file| lookup(file, key));
    if let Ok(Some(value)) = value {
        value
    } else {
        unsafe {
            *libc::__errno_location() = libc::ENOENT;
        }
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn auxv(entries: &[(libc::c_ulong, libc::c_ulong)]) -> Vec<u8> {
        entries
            .iter()
            .flat_map(|(key, value)| key.to_ne_bytes().into_iter().chain(value.to_ne_bytes()))
            .collect()
    }

    #[test]
    fn finds_hardware_capabilities() {
        let data = auxv(&[(6, 4096), (16, 0x1000), (0, 0)]);
        assert_eq!(lookup(&data[..], 16).unwrap(), Some(0x1000));
    }

    #[test]
    fn distinguishes_zero_from_missing() {
        let data = auxv(&[(16, 0), (0, 0)]);
        assert_eq!(lookup(&data[..], 16).unwrap(), Some(0));
        assert_eq!(lookup(&data[..], 17).unwrap(), None);
    }

    #[test]
    fn stops_at_null_entry() {
        let data = auxv(&[(0, 0), (16, 0x1000)]);
        assert_eq!(lookup(&data[..], 16).unwrap(), None);
    }

    #[test]
    fn rejects_truncated_entries() {
        let data = auxv(&[(16, 0x1000)]);
        assert!(lookup(&data[..data.len() - 1], 16).is_err());
        assert!(lookup(&[][..], 16).is_err());
    }
}
