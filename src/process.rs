use fork::Fork;

use crate::error::Error;

pub fn execvp<T>(args: &[T])
    where T: AsRef<str>
{
    if let Ok(Fork::Child) = fork::fork() {
        fork::setsid().expect("setsid failed");

        /* swap to const pointers. into_raw() can leak here
         * because we will execvp() or unreachable!() */
        let mut cs: Vec<_> = args
            .iter()
            .map(|x| {
                std::ffi::CString::new(x.as_ref())
                    .expect("spawn: invalid arguments")
                    .into_raw()
            })
            .collect();

        /* null ptr terminate the list */
        cs.push(std::ptr::null_mut());

        unsafe {
            libc::execvp(cs[0], (&cs[..]).as_ptr() as *const *const i8);
        }

        eprintln!("failed to spawn process");
        std::process::exit(1);
    }
}

pub fn reap() -> Result<bool, Error> {
    let mut zombie = false;

    loop {
        let rv = unsafe {
            libc::waitpid(
                -1,
                std::ptr::null::<*const i32>() as *mut i32,
                libc::WNOHANG,
            )
        };

        if rv < 0 {
            let e = std::io::Error::last_os_error();
            let errno = std::io::Error::raw_os_error(&e);
            match errno {
                Some(libc::ECHILD) => break Ok(zombie),
                Some(_) => break Err(Error::IoError(e)),
                None => unreachable!(),
            }
        } else if rv == 0 {
            break Ok(zombie);
        } else {
            zombie = true;
        }
    }
}
