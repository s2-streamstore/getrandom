// Copyright 2018 Developers of the Rand project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Implementation for Linux / Android
extern crate libc;

use super::Error;
use super::utils::use_init;
use std::fs::File;
use std::io;
use std::io::Read;
use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, ATOMIC_BOOL_INIT, Ordering};

static RNG_INIT: AtomicBool = ATOMIC_BOOL_INIT;

enum RngSource {
    GetRandom,
    Device(File),
}

thread_local!(
    static RNG_SOURCE: RefCell<Option<RngSource>> = RefCell::new(None);
);

fn syscall_getrandom(dest: &mut [u8]) -> Result<(), io::Error> {
    let ret = unsafe {
        libc::syscall(libc::SYS_getrandom, dest.as_mut_ptr(), dest.len(), 0)
    };
    if ret == -1 || ret != dest.len() as i64 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

pub fn getrandom(dest: &mut [u8]) -> Result<(), Error> {
    RNG_SOURCE.with(|f| {
        use_init(f,
        || {
            let s = if is_getrandom_available() {
                RngSource::GetRandom
            } else {
                // read one byte from "/dev/random" to ensure that
                // OS RNG has initialized
                if !RNG_INIT.load(Ordering::Relaxed) {
                    File::open("/dev/random")?.read_exact(&mut [0u8; 1])?;
                    RNG_INIT.store(true, Ordering::Relaxed)
                }
                RngSource::Device(File::open("/dev/urandom")?)
            };
            Ok(s)
        }, |f| {
            match f {
                RngSource::GetRandom => syscall_getrandom(dest),
                RngSource::Device(f) => f.read_exact(dest),
            }
        }).map_err(|_| Error::Unknown)
    })
}

fn is_getrandom_available() -> bool {
    use std::sync::{Once, ONCE_INIT};

    static CHECKER: Once = ONCE_INIT;
    static AVAILABLE: AtomicBool = ATOMIC_BOOL_INIT;

    CHECKER.call_once(|| {
        let mut buf: [u8; 0] = [];
        let available = match syscall_getrandom(&mut buf) {
            Ok(()) => true,
            Err(err) => err.raw_os_error() != Some(libc::ENOSYS),
        };
        AVAILABLE.store(available, Ordering::Relaxed);
    });

    AVAILABLE.load(Ordering::Relaxed)
}
